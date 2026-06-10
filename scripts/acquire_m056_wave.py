#!/usr/bin/env python3
"""Acquire M056 Wave 1 arXiv PDFs with bounded retry.

This script is an operational acquisition probe only. It downloads PDFs into the
local article catalog, records explicit per-PDF statuses, and emits a wave-local
acquisition log plus corpus manifest. It does not import, promote, or write graph
data; all safety defaults remain false.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
import time
import urllib.error
import urllib.parse
import urllib.request
from collections import Counter
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m056-bfs-wave-acquisition.v1"
DEFAULT_WAVE_ORDER = Path("/tmp/wave-order.json")
DEFAULT_OUTPUT_DIR = Path("artifacts/m056-bfs-graph/wave-1")
DEFAULT_ARTICLE_CATALOG_ROOT = Path("data/article_catalog/article_catalog/arxiv")
DEFAULT_MAX_RETRIES = 3
DEFAULT_TIMEOUT_SECONDS = 30
DEFAULT_MINIMUM_ACQUIRED = 25
DEFAULT_TARGET_COUNT = 30
ANCHOR_ARXIV_ID = "2605.18747"
USER_AGENT = "daily-archive-m056-wave-acquisition/1.0"
PDF_PAGE_RE = re.compile(rb"/Type\s*/Page\b")
ARXIV_ID_RE = re.compile(r"^\d{4}\.\d{4,5}(?:v\d+)?$")
SUPPORTED_CATEGORIES = {"cs-ai", "cs-cl", "cs-cv", "cs-lg"}

# The task-level Wave 1 contract intentionally overrides /tmp/wave-order.json,
# whose first-mentioned order currently differs after the anchor.
WAVE_1_ARXIV_IDS = [
    "2107.03374",
    "2108.07732",
    "2203.13474",
    "2310.06770",
    "2211.12588",
    "2312.04474",
    "2204.01691",
    "2308.03688",
    "2603.28052",
    "2603.03329",
    "2603.03836",
    "2603.04177",
    "2603.04257",
    "2603.05621",
    "2603.11226",
    "2603.13258",
    "2603.19329",
    "2603.21430",
    "2603.21520",
    "2603.24533",
    "2603.25723",
    "2603.26664",
    "2603.28119",
    "2604.08224",
    "2604.11839",
    "2604.14228",
    "2604.25850",
    "2601.03515",
    "2601.05808",
    "2601.06789",
]


def _utc_now() -> str:
    return dt.datetime.now(tz=dt.timezone.utc).isoformat()


def _safety_defaults() -> dict[str, bool]:
    return {
        "graph_write_allowed": False,
        "production_import_attempted": False,
        "promotion_allowed": False,
        "facts_promoted": False,
        "external_mutation_allowed": False,
    }


def _atomic_write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_suffix(path.suffix + ".tmp")
    tmp_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp_path.replace(path)


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _estimate_pdf_pages(path: Path) -> int:
    try:
        data = path.read_bytes()
    except OSError:
        return 0
    return max(1, len(PDF_PAGE_RE.findall(data)))


def _load_wave_order(path: Path) -> list[str]:
    if not path.exists():
        return []
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, list):
        raise ValueError(f"wave order must be a JSON list: {path}")
    return [str(item) for item in data]


def _normalize_category(primary_category: str | None) -> str:
    if not primary_category:
        return "mixed-source"
    normalized = primary_category.lower().replace(".", "-")
    return normalized if normalized in SUPPORTED_CATEGORIES else "mixed-source"


def _fetch_arxiv_categories(arxiv_ids: list[str], *, timeout: int) -> dict[str, str]:
    """Best-effort arXiv API category lookup; failures fall back to mixed-source."""
    categories = {arxiv_id: "mixed-source" for arxiv_id in arxiv_ids}
    if not arxiv_ids:
        return categories
    query = urllib.parse.urlencode({"id_list": ",".join(arxiv_ids), "max_results": str(len(arxiv_ids))})
    request = urllib.request.Request(
        f"https://export.arxiv.org/api/query?{query}",
        headers={"User-Agent": USER_AGENT},
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            text = response.read().decode("utf-8", errors="replace")
    except (OSError, urllib.error.URLError, TimeoutError):
        return categories

    for entry_text in re.findall(r"<entry>(.*?)</entry>", text, flags=re.DOTALL):
        id_match = re.search(r"<id>https?://arxiv\.org/abs/([^<]+)</id>", entry_text)
        cat_match = re.search(r"<arxiv:primary_category[^>]+term=\"([^\"]+)\"", entry_text)
        if not id_match:
            continue
        arxiv_id = id_match.group(1).split("v", 1)[0]
        categories[arxiv_id] = _normalize_category(cat_match.group(1) if cat_match else None)
    return categories


def _target_pdf_path(article_catalog_root: Path, category: str, arxiv_id: str) -> Path:
    return article_catalog_root / category / arxiv_id / "source" / f"{arxiv_id}.pdf"


def _download_pdf(
    arxiv_id: str,
    dest_path: Path,
    *,
    max_retries: int,
    timeout: int,
) -> dict[str, Any]:
    url = f"https://arxiv.org/pdf/{arxiv_id}"
    attempts: list[dict[str, Any]] = []
    last_error: str | None = None
    last_http_status: int | None = None

    for attempt_number in range(1, max_retries + 1):
        started_at = _utc_now()
        try:
            request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
            with urllib.request.urlopen(request, timeout=timeout) as response:
                body = response.read()
                http_status = int(getattr(response, "status", 0) or 0)
                content_type = response.headers.get("content-type", "")
            last_http_status = http_status
            is_pdf = body.startswith(b"%PDF") or "pdf" in content_type.lower()
            attempts.append(
                {
                    "attempt": attempt_number,
                    "started_at": started_at,
                    "http_status": http_status,
                    "bytes": len(body),
                    "content_type": content_type,
                    "status": "acquired" if http_status == 200 and is_pdf else "blocked",
                    "error": None if is_pdf else "response was not a PDF",
                }
            )
            if http_status == 200 and is_pdf:
                dest_path.parent.mkdir(parents=True, exist_ok=True)
                tmp_path = dest_path.with_suffix(dest_path.suffix + ".tmp")
                tmp_path.write_bytes(body)
                tmp_path.replace(dest_path)
                return {"status": "acquired", "attempts": attempts, "http_status": http_status, "error": None}
            if http_status == 404:
                return {"status": "blocked", "attempts": attempts, "http_status": http_status, "error": "HTTP 404"}
            last_error = f"HTTP {http_status}"
        except urllib.error.HTTPError as exc:
            last_http_status = int(exc.code)
            body = exc.read() if hasattr(exc, "read") else b""
            last_error = f"HTTP {exc.code}"
            attempts.append(
                {
                    "attempt": attempt_number,
                    "started_at": started_at,
                    "http_status": int(exc.code),
                    "bytes": len(body),
                    "status": "blocked" if exc.code == 404 else "network_error",
                    "error": last_error,
                }
            )
            if exc.code == 404:
                return {"status": "blocked", "attempts": attempts, "http_status": int(exc.code), "error": last_error}
        except (OSError, urllib.error.URLError, TimeoutError) as exc:
            last_error = f"{type(exc).__name__}: {exc}"
            attempts.append(
                {
                    "attempt": attempt_number,
                    "started_at": started_at,
                    "http_status": None,
                    "bytes": 0,
                    "status": "network_error",
                    "error": last_error,
                }
            )
        if attempt_number < max_retries:
            time.sleep(min(2 ** (attempt_number - 1), 4))

    return {
        "status": "network_error" if last_http_status != 404 else "blocked",
        "attempts": attempts,
        "http_status": last_http_status,
        "error": last_error or "download failed",
    }


def _entry_from_existing_pdf(arxiv_id: str, category: str, pdf_path: Path) -> dict[str, Any]:
    return {
        "status": "acquired",
        "arxiv_id": arxiv_id,
        "requested_arxiv_id": arxiv_id,
        "category": category,
        "path": str(pdf_path.as_posix()),
        "pdf_url": f"https://arxiv.org/pdf/{arxiv_id}",
        "sha256": _sha256(pdf_path),
        "bytes": pdf_path.stat().st_size,
        "pages_estimate": _estimate_pdf_pages(pdf_path),
        "attempts": [],
        "http_status": 200,
        "error": None,
        "used_existing_file": True,
    }


def _alternative_candidates(wave_order: list[str], requested_ids: list[str], failed_id: str) -> list[str]:
    prefix = failed_id.split(".", 1)[0]
    requested = set(requested_ids)
    candidates = [item for item in wave_order if item not in requested and item != ANCHOR_ARXIV_ID and item.startswith(prefix + ".")]
    return [item for item in candidates if ARXIV_ID_RE.match(item)]


def _manifest_from_entries(
    entries: list[dict[str, Any]],
    output_dir: Path,
    *,
    source_milestone: str,
    schema_version: str,
    source_label: str,
) -> dict[str, Any]:
    pdfs = []
    for entry in entries:
        if entry.get("status") != "acquired":
            continue
        pdfs.append(
            {
                "arxiv_id": entry["arxiv_id"],
                "requested_arxiv_id": entry.get("requested_arxiv_id", entry["arxiv_id"]),
                "category": entry.get("category", "mixed-source"),
                "path": entry["path"],
                "sha256": entry.get("sha256"),
                "size_bytes": entry.get("bytes", 0),
                "pages_estimate": entry.get("pages_estimate", 0),
                "source_milestone": source_milestone,
            }
        )
    return {
        "schema_version": schema_version,
        "generated_at": _utc_now(),
        "source": source_label,
        "pdf_count": len(pdfs),
        "minimum_expected_pdfs": DEFAULT_MINIMUM_ACQUIRED,
        "expected_total_pdfs": DEFAULT_TARGET_COUNT,
        "output_dir": str(output_dir.as_posix()),
        "safety_defaults": _safety_defaults(),
        "pdfs": pdfs,
    }


def acquire_wave(
    *,
    wave_order_path: Path,
    output_dir: Path,
    article_catalog_root: Path,
    max_retries: int,
    timeout: int,
    use_task_wave_ids: bool,
    source_milestone: str,
    manifest_schema_version: str,
    manifest_source_label: str,
) -> dict[str, Any]:
    wave_order = _load_wave_order(wave_order_path)
    requested_ids = WAVE_1_ARXIV_IDS if use_task_wave_ids else [item for item in wave_order if item != ANCHOR_ARXIV_ID][:DEFAULT_TARGET_COUNT]
    if len(requested_ids) != DEFAULT_TARGET_COUNT:
        raise ValueError(f"expected {DEFAULT_TARGET_COUNT} requested IDs, got {len(requested_ids)}")
    invalid_ids = [item for item in requested_ids if not ARXIV_ID_RE.match(item)]
    if invalid_ids:
        raise ValueError(f"invalid arXiv IDs: {invalid_ids}")

    category_map = _fetch_arxiv_categories(requested_ids, timeout=timeout)
    entries: list[dict[str, Any]] = []
    used_alternatives: set[str] = set()

    for requested_id in requested_ids:
        candidate_ids = [requested_id]
        entry: dict[str, Any] | None = None
        for candidate_id in candidate_ids:
            category = category_map.get(candidate_id, "mixed-source")
            pdf_path = _target_pdf_path(article_catalog_root, category, candidate_id)
            if pdf_path.exists() and pdf_path.stat().st_size > 0:
                entry = _entry_from_existing_pdf(candidate_id, category, pdf_path)
                entry["requested_arxiv_id"] = requested_id
                break
            result = _download_pdf(candidate_id, pdf_path, max_retries=max_retries, timeout=timeout)
            entry = {
                "status": result["status"],
                "arxiv_id": candidate_id,
                "requested_arxiv_id": requested_id,
                "category": category,
                "path": str(pdf_path.as_posix()),
                "pdf_url": f"https://arxiv.org/pdf/{candidate_id}",
                "sha256": _sha256(pdf_path) if result["status"] == "acquired" and pdf_path.exists() else None,
                "bytes": pdf_path.stat().st_size if result["status"] == "acquired" and pdf_path.exists() else 0,
                "pages_estimate": _estimate_pdf_pages(pdf_path) if result["status"] == "acquired" and pdf_path.exists() else 0,
                "attempts": result["attempts"],
                "http_status": result["http_status"],
                "error": result["error"],
                "used_existing_file": False,
            }
            if entry["status"] == "acquired":
                break

        if entry is not None and entry["status"] == "blocked":
            for alt_id in _alternative_candidates(wave_order, requested_ids, requested_id):
                if alt_id in used_alternatives:
                    continue
                alt_category = _fetch_arxiv_categories([alt_id], timeout=timeout).get(alt_id, "mixed-source")
                alt_path = _target_pdf_path(article_catalog_root, alt_category, alt_id)
                alt_result = _download_pdf(alt_id, alt_path, max_retries=max_retries, timeout=timeout)
                entry.setdefault("alternative_attempts", []).append(
                    {
                        "arxiv_id": alt_id,
                        "status": alt_result["status"],
                        "http_status": alt_result["http_status"],
                        "error": alt_result["error"],
                        "attempts": alt_result["attempts"],
                    }
                )
                if alt_result["status"] == "acquired":
                    used_alternatives.add(alt_id)
                    entry = {
                        "status": "acquired",
                        "arxiv_id": alt_id,
                        "requested_arxiv_id": requested_id,
                        "category": alt_category,
                        "path": str(alt_path.as_posix()),
                        "pdf_url": f"https://arxiv.org/pdf/{alt_id}",
                        "sha256": _sha256(alt_path),
                        "bytes": alt_path.stat().st_size,
                        "pages_estimate": _estimate_pdf_pages(alt_path),
                        "attempts": alt_result["attempts"],
                        "http_status": alt_result["http_status"],
                        "error": None,
                        "used_existing_file": False,
                        "replaced_blocked_arxiv_id": requested_id,
                    }
                    break
                break
        entries.append(entry if entry is not None else {"status": "network_error", "arxiv_id": requested_id, "error": "no attempt recorded"})

    status_counts = Counter(str(entry.get("status", "network_error")) for entry in entries)
    category_counts = Counter(str(entry.get("category", "mixed-source")) for entry in entries if entry.get("status") == "acquired")
    log = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "inputs": {
            "wave_order_path": str(wave_order_path),
            "article_catalog_root": str(article_catalog_root.as_posix()),
            "source_order": "task_explicit_wave_1_ids" if use_task_wave_ids else "wave_order_after_self_skip",
        },
        "anchor_arxiv_id": ANCHOR_ARXIV_ID,
        "requested_count": len(requested_ids),
        "minimum_acquired": DEFAULT_MINIMUM_ACQUIRED,
        "success_threshold_percent": 90,
        "requested_arxiv_ids": requested_ids,
        "status_counts": dict(sorted(status_counts.items())),
        "success_count": status_counts.get("acquired", 0),
        "blocked_count": status_counts.get("blocked", 0),
        "network_error_count": status_counts.get("network_error", 0),
        "category_counts": dict(sorted(category_counts.items())),
        "safety_defaults": _safety_defaults(),
        "entries": entries,
    }
    _atomic_write_json(output_dir / "acquisition-log.json", log)
    _atomic_write_json(
        output_dir / "corpus-manifest.json",
        _manifest_from_entries(
            entries,
            output_dir,
            source_milestone=source_milestone,
            schema_version=manifest_schema_version,
            source_label=manifest_source_label,
        ),
    )
    return log


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wave-order", type=Path, default=DEFAULT_WAVE_ORDER)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--article-catalog-root", type=Path, default=DEFAULT_ARTICLE_CATALOG_ROOT)
    parser.add_argument("--max-retries", type=int, default=DEFAULT_MAX_RETRIES)
    parser.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT_SECONDS)
    parser.add_argument("--wave-order-source", action="store_true", help="Use /tmp/wave-order.json after self-skip instead of task explicit Wave 1 IDs.")
    parser.add_argument("--source-milestone", default="M056-lchpnp/S01")
    parser.add_argument("--manifest-schema-version", default="m056-bfs-wave-1-corpus-manifest.v1")
    parser.add_argument("--manifest-source-label", default="M056-lchpnp S01 Wave 1 acquisition")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    log = acquire_wave(
        wave_order_path=args.wave_order,
        output_dir=args.output_dir,
        article_catalog_root=args.article_catalog_root,
        max_retries=args.max_retries,
        timeout=args.timeout,
        use_task_wave_ids=not args.wave_order_source,
        source_milestone=args.source_milestone,
        manifest_schema_version=args.manifest_schema_version,
        manifest_source_label=args.manifest_source_label,
    )
    print(json.dumps(log, indent=2, sort_keys=True))
    return 0 if int(log.get("success_count") or 0) >= DEFAULT_MINIMUM_ACQUIRED else 2


if __name__ == "__main__":
    raise SystemExit(main())
