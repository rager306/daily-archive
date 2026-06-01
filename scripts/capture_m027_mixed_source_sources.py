#!/usr/bin/env python3
"""Fail-closed source acquisition boundary for the M027 mixed-source corpus.

This module is intentionally safe to import from tests.  It performs acquisition
only: selected catalog article records are loaded through ``index.json``, source
variant roles are mapped to fixed catalog-local target paths, fetched bytes are
validated and hashed, and metadata-only result records are emitted.  Failed
fetches never become fallback source artifacts.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import time
import urllib.error
import urllib.request
from collections.abc import Callable, Iterable, Mapping
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

USER_AGENT = "daily-archive-m027-source-acquisition/0.1 (metadata-only boundary)"
NETWORK_TIMEOUT_SECONDS = 25
SELECTION_ID = "m027-mixed-source-corpus-v1"
SOURCE_ACQUISITION_SCHEMA_VERSION = "m027-source-acquisition.v1"

ROLE_TARGETS: dict[str, str] = {
    "arxiv_abs_page": "source/abs.html",
    "arxiv_pdf": "source/original.pdf",
    "nature_html": "source/article.html",
    "publisher_html": "source/article.html",
}

PDF_ROLES = {"arxiv_pdf", "publisher_pdf"}
HTML_MEDIA_TYPES = {
    "arxiv_abs_page": "text/html",
    "nature_html": "text/html",
    "publisher_html": "text/html",
}

FAIL_CLOSED_SAFETY_FLAGS: dict[str, bool] = {
    "metadata_manifests_embed_raw_text": False,
    "metadata_manifests_embed_raw_binary": False,
    "graph_import_allowed": False,
    "production_ladybugdb_write_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
    "raw_text_embedded_in_metadata": False,
    "raw_binary_embedded_in_metadata": False,
}

FORBIDDEN_RESULT_KEYS = {
    "text",
    "raw_text",
    "html",
    "pdf",
    "binary",
    "bytes",
    "base64",
    "payload",
    "content",
    "body",
}


@dataclass(frozen=True)
class FetchResponse:
    """Small response envelope accepted from injectable fetchers."""

    data: bytes
    media_type: str | None = None
    status_code: int | None = None
    final_url: str | None = None


Fetcher = Callable[[str], bytes | FetchResponse]


def utc_now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        payload = json.load(handle)
    if not isinstance(payload, dict):
        raise ValueError(f"expected JSON object: {path}")
    return payload


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=False) + "\n", encoding="utf-8")


def append_jsonl(path: Path, rows: Iterable[Mapping[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True) + "\n")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def safe_catalog_path(root: Path, rel_path: str) -> Path:
    if not isinstance(rel_path, str) or not rel_path.strip():
        raise ValueError("empty_catalog_relative_path")
    if "://" in rel_path:
        raise ValueError("url_not_allowed_as_local_path")
    normalized = PurePosixPath(rel_path.replace("\\", "/"))
    if normalized.is_absolute() or ".." in normalized.parts or any(part == "" for part in normalized.parts):
        raise ValueError("unsafe_catalog_relative_path")
    root_resolved = root.resolve()
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise ValueError("catalog_path_escapes_root")
    return resolved


def target_path_for_variant(article_dir: Path, variant: Mapping[str, Any]) -> tuple[Path | None, str | None, str | None]:
    role = variant.get("source_role")
    if not isinstance(role, str) or role not in ROLE_TARGETS:
        return None, None, "unsupported_source_role"

    expected_rel = ROLE_TARGETS[role]
    supplied_path = variant.get("path")
    if supplied_path is not None:
        if not isinstance(supplied_path, str):
            return None, expected_rel, "malformed_source_path"
        try:
            supplied_resolved = safe_catalog_path(article_dir, supplied_path)
        except ValueError as exc:
            return None, expected_rel, str(exc)
        expected_resolved = safe_catalog_path(article_dir, expected_rel)
        if supplied_resolved != expected_resolved:
            return None, expected_rel, "unexpected_source_path_for_role"

    try:
        return safe_catalog_path(article_dir, expected_rel), expected_rel, None
    except ValueError as exc:  # Defensive: ROLE_TARGETS is constant but still fail closed.
        return None, expected_rel, str(exc)


def default_fetcher(url: str) -> FetchResponse:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=NETWORK_TIMEOUT_SECONDS) as response:  # noqa: S310 - explicit acquisition boundary.
        media_type = response.headers.get_content_type() if response.headers else None
        status_code = getattr(response, "status", None)
        final_url = response.geturl()
        return FetchResponse(response.read(), media_type=media_type, status_code=status_code, final_url=final_url)


def fixture_response_fetcher(response_dir: Path) -> Fetcher:
    """Return a fetcher that reads bytes from files named by URL SHA-256.

    Tests and offline replays can place ``<sha256(url)>.bin`` files in the
    response directory.  Missing files raise ``FileNotFoundError`` and therefore
    produce blocked diagnostics instead of fallback captures.
    """

    def fetch(url: str) -> FetchResponse:
        key = sha256_bytes(url.encode("utf-8"))
        candidates = [response_dir / f"{key}.bin", response_dir / key]
        for candidate in candidates:
            if candidate.exists():
                return FetchResponse(candidate.read_bytes())
        raise FileNotFoundError(f"fixture response missing for URL hash {key}")

    return fetch


def normalize_fetch_response(value: bytes | FetchResponse) -> FetchResponse:
    if isinstance(value, FetchResponse):
        return value
    if isinstance(value, bytes):
        return FetchResponse(value)
    raise TypeError(f"fetcher returned unsupported response type: {type(value).__name__}")


def diagnostic_result(
    *,
    article_ref: str | None,
    variant: Mapping[str, Any],
    status: str,
    diagnostic_code: str,
    failure_reason: str,
    local_path: str | None = None,
    media_type: str | None = None,
    network_fetch_attempted: bool = False,
    command_provenance: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    role = variant.get("source_role")
    result = {
        "schema_version": SOURCE_ACQUISITION_SCHEMA_VERSION,
        "selection_id": SELECTION_ID,
        "article_ref": article_ref,
        "article_key": None,
        "variant_id": variant.get("variant_id") if isinstance(variant.get("variant_id"), str) else None,
        "source_role": role if isinstance(role, str) else None,
        "url_role": role if isinstance(role, str) else None,
        "url": variant.get("url") if isinstance(variant.get("url"), str) else None,
        "status": status,
        "diagnostic_code": diagnostic_code,
        "failure_reason": failure_reason,
        "local_path": local_path,
        "sha256": None,
        "byte_size": 0,
        "media_type": media_type,
        "network_fetch_attempted": network_fetch_attempted,
        "captured_at": None,
        "command_provenance": dict(command_provenance or {}),
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "raw_payload_embedded_in_metadata": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }
    assert not (set(result) & FORBIDDEN_RESULT_KEYS)
    return result


def captured_result(
    *,
    article: Mapping[str, Any],
    variant: Mapping[str, Any],
    local_path: str,
    data: bytes,
    media_type: str,
    network_fetch_attempted: bool,
    command_provenance: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    role = str(variant["source_role"])
    article_ref = article.get("catalog_path") if isinstance(article.get("catalog_path"), str) else None
    result = {
        "schema_version": SOURCE_ACQUISITION_SCHEMA_VERSION,
        "selection_id": SELECTION_ID,
        "article_ref": article_ref,
        "article_key": article.get("article_key") if isinstance(article.get("article_key"), str) else None,
        "variant_id": variant.get("variant_id") if isinstance(variant.get("variant_id"), str) else None,
        "source_role": role,
        "url_role": role,
        "url": variant.get("url") if isinstance(variant.get("url"), str) else None,
        "status": "captured",
        "diagnostic_code": "captured_source_artifact",
        "failure_reason": None,
        "local_path": local_path,
        "sha256": sha256_bytes(data),
        "byte_size": len(data),
        "media_type": media_type,
        "network_fetch_attempted": network_fetch_attempted,
        "captured_at": utc_now(),
        "command_provenance": dict(command_provenance or {}),
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "raw_payload_embedded_in_metadata": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }
    assert not (set(result) & FORBIDDEN_RESULT_KEYS)
    return result


def validate_response_bytes(role: str, data: bytes) -> tuple[bool, str | None, str | None]:
    if not data:
        return False, "empty_response", "response body was empty"
    if role in PDF_ROLES and not data.startswith(b"%PDF-"):
        return False, "bad_pdf_signature", "PDF source did not start with %PDF-"
    return True, None, None


def expected_media_type(role: str, response: FetchResponse) -> str:
    if role in PDF_ROLES:
        return "application/pdf"
    return response.media_type or HTML_MEDIA_TYPES.get(role, "application/octet-stream")


def capture_variant(
    article_path: Path,
    article: Mapping[str, Any],
    variant: Mapping[str, Any],
    *,
    fetcher: Fetcher = default_fetcher,
    write: bool = True,
    command_provenance: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Capture one source variant or return a metadata-only blocked/failed result."""

    article_dir = article_path.parent
    article_ref = article.get("catalog_path") if isinstance(article.get("catalog_path"), str) else None
    target, local_path, path_error = target_path_for_variant(article_dir, variant)
    if path_error is not None:
        return diagnostic_result(
            article_ref=article_ref,
            variant=variant,
            status="blocked",
            diagnostic_code=path_error,
            failure_reason="source variant target path is not allowed",
            local_path=local_path,
            media_type=variant.get("media_type") if isinstance(variant.get("media_type"), str) else None,
            command_provenance=command_provenance,
        )

    url = variant.get("url")
    if not isinstance(url, str) or not url.strip():
        return diagnostic_result(
            article_ref=article_ref,
            variant=variant,
            status="blocked",
            diagnostic_code="missing_source_url",
            failure_reason="source variant URL is missing",
            local_path=local_path,
            media_type=variant.get("media_type") if isinstance(variant.get("media_type"), str) else None,
            command_provenance=command_provenance,
        )

    role = str(variant["source_role"])
    try:
        response = normalize_fetch_response(fetcher(url))
    except TimeoutError as exc:
        return diagnostic_result(
            article_ref=article_ref,
            variant=variant,
            status="blocked",
            diagnostic_code="fetch_timeout",
            failure_reason=str(exc) or "fetch timed out",
            local_path=local_path,
            media_type=variant.get("media_type") if isinstance(variant.get("media_type"), str) else None,
            network_fetch_attempted=True,
            command_provenance=command_provenance,
        )
    except (urllib.error.URLError, OSError, ValueError, TypeError) as exc:
        return diagnostic_result(
            article_ref=article_ref,
            variant=variant,
            status="blocked",
            diagnostic_code="fetch_failed",
            failure_reason=f"{type(exc).__name__}: {exc}",
            local_path=local_path,
            media_type=variant.get("media_type") if isinstance(variant.get("media_type"), str) else None,
            network_fetch_attempted=True,
            command_provenance=command_provenance,
        )

    ok, diagnostic_code, failure_reason = validate_response_bytes(role, response.data)
    if not ok:
        return diagnostic_result(
            article_ref=article_ref,
            variant=variant,
            status="failed",
            diagnostic_code=str(diagnostic_code),
            failure_reason=str(failure_reason),
            local_path=local_path,
            media_type=expected_media_type(role, response),
            network_fetch_attempted=True,
            command_provenance=command_provenance,
        )

    if write:
        assert target is not None  # for type-checkers; path_error guard proves this.
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(response.data)

    return captured_result(
        article=article,
        variant=variant,
        local_path=str(local_path),
        data=response.data,
        media_type=expected_media_type(role, response),
        network_fetch_attempted=True,
        command_provenance=command_provenance,
    )


def selected_article_paths(catalog_root: Path, index: Mapping[str, Any], selection: Mapping[str, Any]) -> list[Path]:
    rows = index.get("articles")
    selected = selection.get("articles")
    if not isinstance(rows, list) or not isinstance(selected, list):
        raise ValueError("malformed index or selection articles")
    by_ref = {
        row.get("article_ref"): row
        for row in rows
        if isinstance(row, dict) and isinstance(row.get("article_ref"), str)
    }
    paths: list[Path] = []
    for selected_row in selected:
        article_ref = selected_row.get("article_ref") if isinstance(selected_row, dict) else None
        if not isinstance(article_ref, str) or article_ref not in by_ref:
            raise ValueError(f"selection article not present in index: {article_ref}")
        article_path = by_ref[article_ref].get("article_path")
        if not isinstance(article_path, str):
            raise ValueError(f"index row missing article_path: {article_ref}")
        paths.append(safe_catalog_path(catalog_root, article_path))
    return paths


def command_provenance(argv: list[str]) -> dict[str, Any]:
    try:
        commit = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            check=False,
            capture_output=True,
            text=True,
            timeout=5,
        ).stdout.strip()
    except Exception:
        commit = None
    return {
        "argv": argv,
        "cwd": str(Path.cwd()),
        "git_commit": commit or None,
    }


def build_summary(results: list[dict[str, Any]], *, provenance: Mapping[str, Any]) -> dict[str, Any]:
    counts: dict[str, int] = {"captured": 0, "blocked": 0, "failed": 0}
    for result in results:
        status = str(result.get("status"))
        if status in counts:
            counts[status] += 1
    return {
        "schema_version": SOURCE_ACQUISITION_SCHEMA_VERSION,
        "selection_id": SELECTION_ID,
        "status": "completed_with_diagnostics" if counts["blocked"] or counts["failed"] else "captured",
        "variant_count": len(results),
        "counts": counts,
        "results": results,
        "command_provenance": dict(provenance),
        "input_hashes": {},
        "output_hashes": {},
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "raw_payload_embedded_in_metadata": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }


def capture_selection(
    *,
    catalog_root: Path,
    index_path: Path,
    selection_path: Path,
    fetcher: Fetcher = default_fetcher,
    write: bool = True,
    provenance: Mapping[str, Any] | None = None,
) -> list[dict[str, Any]]:
    index = load_json(index_path)
    selection = load_json(selection_path)
    results: list[dict[str, Any]] = []
    for article_path in selected_article_paths(catalog_root, index, selection):
        article = load_json(article_path)
        variants = article.get("source_variants")
        if not isinstance(variants, list):
            raise ValueError(f"article has malformed source_variants: {article_path}")
        for variant in variants:
            if not isinstance(variant, dict):
                raise ValueError(f"article has malformed source variant: {article_path}")
            if variant.get("source_role") in ROLE_TARGETS:
                results.append(
                    capture_variant(
                        article_path,
                        article,
                        variant,
                        fetcher=fetcher,
                        write=write,
                        command_provenance=provenance,
                    )
                )
    return results


def render_report(summary: Mapping[str, Any]) -> str:
    counts = summary.get("counts") if isinstance(summary.get("counts"), dict) else {}
    lines = [
        "# M027 Source Acquisition Report",
        "",
        "This report is metadata-only. It does not embed article text, HTML snippets, PDF text, binary bytes, or base64 payloads.",
        "",
        f"- Selection: `{summary.get('selection_id')}`",
        f"- Status: `{summary.get('status')}`",
        f"- Captured: {counts.get('captured', 0)}",
        f"- Blocked: {counts.get('blocked', 0)}",
        f"- Failed: {counts.get('failed', 0)}",
        "- Graph import allowed: false",
        "- Production LadybugDB write allowed: false",
        "",
        "## Variants",
        "",
    ]
    for result in summary.get("results", []):
        if isinstance(result, dict):
            lines.append(
                f"- `{result.get('article_ref')}` `{result.get('source_role')}`: "
                f"{result.get('status')} ({result.get('diagnostic_code')}) -> `{result.get('local_path')}`"
            )
    return "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog-root", type=Path, default=Path("data/article_catalog"))
    parser.add_argument("--index", type=Path, default=Path("data/article_catalog/index.json"))
    parser.add_argument(
        "--selection",
        type=Path,
        default=Path("data/article_corpora/m027-mixed-source-corpus-v1/selection.json"),
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("data/article_corpora/m027-mixed-source-corpus-v1"),
    )
    parser.add_argument("--fixture-response-dir", type=Path)
    parser.add_argument("--dry-run", action="store_true", help="Validate and summarize without writing captured source bytes.")
    args = parser.parse_args(argv[1:])

    started = time.perf_counter()
    provenance = command_provenance(argv)
    fetcher = fixture_response_fetcher(args.fixture_response_dir) if args.fixture_response_dir else default_fetcher
    results = capture_selection(
        catalog_root=args.catalog_root,
        index_path=args.index,
        selection_path=args.selection,
        fetcher=fetcher,
        write=not args.dry_run,
        provenance=provenance,
    )
    summary = build_summary(results, provenance=provenance)
    summary["duration_ms"] = int((time.perf_counter() - started) * 1000)

    summary_path = args.output_dir / "source-acquisition-summary.json"
    diagnostics_path = args.output_dir / "source-acquisition-diagnostics.jsonl"
    report_path = args.output_dir / "source-acquisition-report.md"
    write_json(summary_path, summary)
    append_jsonl(diagnostics_path, results)
    report_path.write_text(render_report(summary), encoding="utf-8")
    print(json.dumps({"summary_path": str(summary_path), "variant_count": len(results), "counts": summary["counts"]}, sort_keys=True))
    return 0 if not summary["counts"]["failed"] else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
