#!/usr/bin/env python3
"""Build the five-PDF corpus manifest for M055 parser benchmark S01."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m055-parser-benchmark-corpus-manifest.v1"
DEFAULT_TARGET_SUBSET = Path("artifacts/m054-pdf-acquisition/target-subset.json")
DEFAULT_OUTPUT = Path("artifacts/m055-parser-benchmark/corpus-manifest.json")
SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "import_eligible": False,
}
ARXIV_ID_RE = re.compile(r"(?P<id>\d{4}\.\d{4,5}(?:v\d+)?)")


def _utc_now() -> str:
    return dt.datetime.now(dt.UTC).isoformat()


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _arxiv_id_from_path(pdf_path: Path) -> str:
    match = ARXIV_ID_RE.search(pdf_path.name)
    if match:
        return match.group("id")
    for part in reversed(pdf_path.parts):
        match = ARXIV_ID_RE.fullmatch(part)
        if match:
            return match.group("id")
    return pdf_path.stem


def _category_from_path(pdf_path: Path) -> str | None:
    parts = pdf_path.parts
    for index, part in enumerate(parts):
        if part == "arxiv" and index + 1 < len(parts):
            return parts[index + 1]
    return None


def _estimate_pages(pdf_path: Path) -> int:
    try:
        data = pdf_path.read_bytes()
    except OSError:
        return 0
    # This is intentionally a cheap heuristic, not a PDF parser. Avoid counting
    # the /Pages tree object as an individual page where possible.
    page_markers = data.count(b"/Type /Page") - data.count(b"/Type /Pages")
    return max(0, int(page_markers))


def _pdf_metadata(pdf_path: Path) -> dict[str, Any]:
    resolved_path = Path(pdf_path)
    stat = resolved_path.stat()
    return {
        "arxiv_id": _arxiv_id_from_path(resolved_path),
        "path": str(resolved_path),
        "size_bytes": stat.st_size,
        "sha256": _sha256(resolved_path),
        "category": _category_from_path(resolved_path),
        "pages_estimate": _estimate_pages(resolved_path),
    }


def _build_corpus_from_target_subset(target_subset_path: Path) -> list[dict[str, Any]]:
    payload = json.loads(target_subset_path.read_text(encoding="utf-8"))
    records = payload.get("records", [])
    entries: list[dict[str, Any]] = []
    for record in records:
        raw_path = record.get("expected_local_pdf_path") or record.get("local_path")
        if not raw_path:
            raise ValueError(f"target subset record is missing a PDF path: {record!r}")
        pdf_path = Path(raw_path)
        if not pdf_path.is_file():
            raise FileNotFoundError(f"PDF path from target subset does not exist: {pdf_path}")
        metadata = _pdf_metadata(pdf_path)
        if record.get("article_key") and metadata["arxiv_id"] == "original":
            metadata["arxiv_id"] = str(record["article_key"])
        if record.get("category"):
            metadata["category"] = record["category"]
        metadata["target_index"] = record.get("index")
        metadata["article_key"] = record.get("article_key") or metadata["arxiv_id"]
        entries.append(metadata)
    return entries


def build_corpus_manifest(
    target_subset_path: Path, output_path: Path = DEFAULT_OUTPUT
) -> dict[str, Any]:
    entries = _build_corpus_from_target_subset(target_subset_path)
    payload = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "target_subset_path": str(target_subset_path),
        "total_count": len(entries),
        "total_bytes": sum(entry["size_bytes"] for entry in entries),
        "safety": dict(SAFETY_DEFAULTS),
        "pdfs": entries,
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return payload


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-subset", default=str(DEFAULT_TARGET_SUBSET))
    parser.add_argument("--output", default=str(DEFAULT_OUTPUT))
    args = parser.parse_args(argv)

    build_corpus_manifest(Path(args.target_subset), Path(args.output))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
