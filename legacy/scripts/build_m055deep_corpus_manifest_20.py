#!/usr/bin/env python3
"""Build the M055deep 20-PDF corpus manifest.

The manifest combines:
- five M051 PDFs from artifacts/m055-parser-benchmark/corpus-manifest.json
- nine pre-existing M027/M041 PDFs discovered from data/article_catalog
- six M055deep acquisitions from artifacts/m055deep-parser-benchmark/acquisition-log.json

The script is intentionally read-only with respect to corpus inputs. It writes
one reproducibility manifest with hashes and safety defaults; it does not import
or promote content into any graph store.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
from collections import Counter
from pathlib import Path
from typing import Any

from research_graph.application.corpus.manifest_io import write_manifest_json_atomic

SCHEMA_VERSION = "m055deep-parser-benchmark-corpus-manifest-20.v1"
DEFAULT_M051_MANIFEST = Path("artifacts/m055-parser-benchmark/corpus-manifest.json")
DEFAULT_ARTICLE_CATALOG_ROOT = Path("data/article_catalog/article_catalog/arxiv")
DEFAULT_ACQUISITION_LOG = Path("artifacts/m055deep-parser-benchmark/acquisition-log.json")
DEFAULT_OUTPUT = Path("artifacts/m055deep-parser-benchmark/corpus-manifest-20.json")
EXPECTED_TOTAL = 20
MINIMUM_TOTAL = 18
MINIMUM_NEW_ACQUISITIONS = 4

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "import_eligible": False,
}

ARXIV_ID_RE = re.compile(r"(?P<id>\d{4}\.\d{4,5}(?:v\d+)?)")
PDF_PAGE_RE = re.compile(rb"/Type\s*/Page\b")


def _utc_now() -> str:
    return dt.datetime.now(dt.UTC).isoformat()


def _read_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise FileNotFoundError(f"required input not found: {path}")
    return json.loads(path.read_text(encoding="utf-8"))


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _article_key_base(article_key: str) -> str:
    return article_key.split("v", 1)[0] if "v" in article_key else article_key


def _arxiv_id_from_path(path: Path) -> str:
    match = ARXIV_ID_RE.search(str(path))
    if not match:
        raise ValueError(f"could not derive arxiv_id from path: {path}")
    return match.group("id")


def _relative_path(path: Path) -> str:
    return str(path.as_posix())


def _estimate_pdf_pages(path: Path) -> int:
    data = path.read_bytes()
    count = len(PDF_PAGE_RE.findall(data))
    return max(count, 1)


def _entry_for_pdf(
    *,
    arxiv_id: str,
    path: Path,
    category: str,
    source_milestone: str,
    pages_estimate: int | None = None,
) -> dict[str, Any]:
    if not path.exists():
        raise FileNotFoundError(f"PDF not found for {arxiv_id}: {path}")
    return {
        "arxiv_id": arxiv_id,
        "path": _relative_path(path),
        "size_bytes": path.stat().st_size,
        "sha256": _sha256(path),
        "category": category,
        "source_milestone": source_milestone,
        "pages_estimate": pages_estimate
        if pages_estimate is not None
        else _estimate_pdf_pages(path),
    }


def _load_m051_entries(m051_manifest_path: Path) -> list[dict[str, Any]]:
    manifest = _read_json(m051_manifest_path)
    entries: list[dict[str, Any]] = []
    for item in sorted(manifest.get("pdfs", []), key=lambda row: row.get("target_index", 0)):
        arxiv_id = (
            item.get("arxiv_id")
            or item.get("article_key")
            or _arxiv_id_from_path(Path(item["path"]))
        )
        entries.append(
            _entry_for_pdf(
                arxiv_id=arxiv_id,
                path=Path(item["path"]),
                category=item["category"],
                source_milestone="M051",
                pages_estimate=item.get("pages_estimate"),
            )
        )
    return entries


def _load_acquired_entries(acquisition_log_path: Path) -> list[dict[str, Any]]:
    log = _read_json(acquisition_log_path)
    entries: list[dict[str, Any]] = []
    for item in log.get("entries", []):
        if item.get("status") != "acquired":
            continue
        local_path = Path(item["local_path"])
        arxiv_id = item.get("article_key") or _arxiv_id_from_path(local_path)
        entries.append(
            _entry_for_pdf(
                arxiv_id=arxiv_id,
                path=local_path,
                category=item["category"],
                source_milestone="M055deep",
            )
        )
    return sorted(entries, key=lambda row: row["arxiv_id"])


def _discover_m027_m041_entries(
    article_catalog_root: Path,
    *,
    excluded_ids: set[str],
) -> list[dict[str, Any]]:
    entries: list[dict[str, Any]] = []
    excluded_bases = {_article_key_base(value) for value in excluded_ids}
    for pdf_path in sorted(article_catalog_root.glob("*/*/source/*.pdf")):
        article_dir = pdf_path.parent.parent
        category_dir = article_dir.parent
        article_key = article_dir.name
        if article_key in excluded_ids or _article_key_base(article_key) in excluded_bases:
            continue
        entries.append(
            _entry_for_pdf(
                arxiv_id=article_key,
                path=pdf_path,
                category=category_dir.name,
                source_milestone="M027/M041",
            )
        )
    return entries


def build_manifest(
    *,
    m051_manifest_path: Path = DEFAULT_M051_MANIFEST,
    article_catalog_root: Path = DEFAULT_ARTICLE_CATALOG_ROOT,
    acquisition_log_path: Path = DEFAULT_ACQUISITION_LOG,
    generated_at: str | None = None,
) -> dict[str, Any]:
    m051_entries = _load_m051_entries(m051_manifest_path)
    acquired_entries = _load_acquired_entries(acquisition_log_path)

    excluded_ids = {entry["arxiv_id"] for entry in m051_entries + acquired_entries}
    m027_m041_entries = _discover_m027_m041_entries(article_catalog_root, excluded_ids=excluded_ids)

    pdfs = m051_entries + m027_m041_entries + acquired_entries
    source_counts = Counter(entry["source_milestone"] for entry in pdfs)
    category_counts = Counter(entry["category"] for entry in pdfs)

    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at or _utc_now(),
        "inputs": {
            "m051_manifest": _relative_path(m051_manifest_path),
            "article_catalog_root": _relative_path(article_catalog_root),
            "acquisition_log": _relative_path(acquisition_log_path),
        },
        "safety_defaults": dict(SAFETY_DEFAULTS),
        "expected_total": EXPECTED_TOTAL,
        "minimum_total": MINIMUM_TOTAL,
        "minimum_new_acquisitions": MINIMUM_NEW_ACQUISITIONS,
        "actual_total": len(pdfs),
        "actual_new_acquisitions": source_counts.get("M055deep", 0),
        "source_milestone_counts": dict(sorted(source_counts.items())),
        "category_counts": dict(sorted(category_counts.items())),
        "pdfs": pdfs,
    }


def _without_generated_at(payload: dict[str, Any]) -> dict[str, Any]:
    comparable = dict(payload)
    comparable.pop("generated_at", None)
    return comparable


def write_manifest(payload: dict[str, Any], output_path: Path) -> dict[str, Any]:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    stable_payload = dict(payload)
    if output_path.exists():
        existing = json.loads(output_path.read_text(encoding="utf-8"))
        if _without_generated_at(existing) == _without_generated_at(stable_payload):
            stable_payload["generated_at"] = existing.get(
                "generated_at", stable_payload["generated_at"]
            )
    write_manifest_json_atomic(output_path, stable_payload, sort_keys=True)
    return stable_payload


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--m051-manifest", type=Path, default=DEFAULT_M051_MANIFEST)
    parser.add_argument("--article-catalog-root", type=Path, default=DEFAULT_ARTICLE_CATALOG_ROOT)
    parser.add_argument("--acquisition-log", type=Path, default=DEFAULT_ACQUISITION_LOG)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args(argv)

    payload = build_manifest(
        m051_manifest_path=args.m051_manifest,
        article_catalog_root=args.article_catalog_root,
        acquisition_log_path=args.acquisition_log,
    )
    write_manifest(payload, args.output)
    print(
        json.dumps(
            {
                "output": str(args.output),
                "actual_total": payload["actual_total"],
                "actual_new_acquisitions": payload["actual_new_acquisitions"],
                "source_milestone_counts": payload["source_milestone_counts"],
                "category_counts": payload["category_counts"],
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
