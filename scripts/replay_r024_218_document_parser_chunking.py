#!/usr/bin/env python3
"""R024/M121 S04: parser+chunking replay on the full 221-article catalog.

Reads canonical article_catalog/index.json directly. For each article, prefer a
local PDF source variant and extract text with PyMuPDF into a reusable offline
cache; otherwise use local HTML/Markdown/TXT content. Then run the existing
FullTextSource -> ingest_full_text -> parse_article -> page-index pipeline.

Fail-closed invariants preserved: no network, no graph writes, no production
import, no LadybugDB writes.
"""

from __future__ import annotations

import json
import sys
from collections import Counter
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import fitz

from research_graph.infrastructure.corpus.ingestion import FullTextSource, ingest_full_text
from research_graph.infrastructure.corpus.parsing.parser import parse_article
from research_graph.infrastructure.papers.indexing.parsed_page_index import (
    build_page_index_from_parsed,
)

REPO_ROOT = Path("/root/daily-archive")
CATALOG_PARENT = REPO_ROOT / "data" / "article_catalog"
CATALOG_ROOT = CATALOG_PARENT / "article_catalog"
INDEX = CATALOG_ROOT / "index.json"
OUTPUT_DIR = REPO_ROOT / "data" / "r024-218-document-corpus-v1" / "parser-chunking"
CACHE_DIR = REPO_ROOT / "data" / "r024-218-document-corpus-v1" / "pdf-text-cache"
EVENTS_LOG = OUTPUT_DIR / "events.jsonl"
SUMMARY = OUTPUT_DIR / "summary.json"


class MetadataOnlySource(RuntimeError):
    """Raised when an article is intentionally metadata-only with no local source."""


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def _resolve_catalog_path(path_value: str | None) -> Path | None:
    if not path_value:
        return None
    path = Path(path_value)
    if path.is_absolute():
        return path
    return CATALOG_PARENT / path


def _find_pdf_source(article: dict[str, Any]) -> Path | None:
    variants = article.get("source_variants", [])
    if not isinstance(variants, list):
        return None
    for variant in variants:
        if not isinstance(variant, dict):
            continue
        if variant.get("source_format") != "pdf":
            continue
        path = _resolve_catalog_path(str(variant.get("path") or ""))
        if path and path.exists():
            return path
    return None


def find_html_source(article_dir: Path) -> Path | None:
    """Find local text/markdown/html source file."""
    for ext in ("*.md", "*.markdown", "*.html", "*.txt"):
        matches = list(article_dir.rglob(ext))
        if matches:
            for preferred in ("abs.html", "article.html"):
                for match in matches:
                    if match.name == preferred:
                        return match
            return matches[0]
    return None


def _extract_pdf_text(pdf_path: Path, cache_path: Path) -> tuple[Path, bool, int, int]:
    """Extract PDF text to cache and return path, reused, chars, pages."""
    if cache_path.exists() and cache_path.stat().st_size > 0:
        text = cache_path.read_text(errors="replace")
        return cache_path, True, len(text), text.count("\n\f\n") + 1

    cache_path.parent.mkdir(parents=True, exist_ok=True)
    page_texts: list[str] = []
    with fitz.open(pdf_path) as doc:
        page_count = doc.page_count
        for page in doc:
            page_text = page.get_text("text")
            page_texts.append(page_text if isinstance(page_text, str) else str(page_text))
    text = "\n\f\n".join(page_texts).strip()
    if not text:
        raise ValueError(f"no extractable text from PDF: {pdf_path}")
    cache_path.write_text(text)
    return cache_path, False, len(text), page_count


def _source_for_article(
    *,
    article_entry: dict[str, Any],
    article: dict[str, Any],
) -> tuple[str, Path, bool, int, int]:
    article_key = str(article_entry["article_key"])
    pdf_path = _find_pdf_source(article)
    if pdf_path is not None:
        cache_path = CACHE_DIR / f"{article_key}.txt"
        text_path, cache_reused, text_chars, pdf_pages = _extract_pdf_text(pdf_path, cache_path)
        return "pdf_converted", text_path, cache_reused, text_chars, pdf_pages

    article_path = CATALOG_PARENT / str(article_entry["article_path"])
    source_path = find_html_source(article_path.parent)
    if source_path is None:
        expected_profile = article.get("expected_profile", {})
        should_load = (
            isinstance(expected_profile, dict) and expected_profile.get("should_load") is False
        )
        if should_load:
            raise MetadataOnlySource("metadata_only_no_local_source_artifact")
        raise FileNotFoundError(f"No local PDF/HTML/text source for {article_path.parent}")
    text_chars = len(source_path.read_text(errors="replace"))
    return "html_native", source_path, True, text_chars, 0


def main() -> int:
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    index = _load_json(INDEX)
    articles = list(index["articles"])
    print(f"Processing {len(articles)} catalog articles...")

    per_article: list[dict[str, Any]] = []
    events: list[dict[str, Any]] = []
    source_kind_counts: Counter[str] = Counter()
    skip_reason_counts: Counter[str] = Counter()
    chunk_counts: list[int] = []
    n_ok = 0
    n_skipped = 0
    n_err = 0

    for entry in articles:
        article_entry = dict(entry)
        ref = str(article_entry["article_ref"])
        key = str(article_entry["article_key"])
        article_path = CATALOG_PARENT / str(article_entry["article_path"])

        try:
            article = _load_json(article_path)
            source_kind, source_path, cache_reused, text_chars, pdf_pages = _source_for_article(
                article_entry=article_entry,
                article=article,
            )
            source = FullTextSource(
                paper_id=ref,
                source_type="text",
                source_path=source_path,
            )
            ingestion = ingest_full_text(source)
            parsed = parse_article(ingestion)
            page_index = build_page_index_from_parsed(parsed)
            n_chunks = len(page_index.nodes) if hasattr(page_index, "nodes") else 0
            if n_chunks <= 0:
                raise ValueError(f"non-positive chunk count: {n_chunks}")

            source_kind_counts[source_kind] += 1
            chunk_counts.append(n_chunks)
            per_article.append(
                {
                    "article_ref": ref,
                    "article_key": key,
                    "source_kind": source_kind,
                    "text_source": str(source_path.relative_to(REPO_ROOT)),
                    "text_chars": text_chars,
                    "pdf_pages": pdf_pages,
                    "cache_reused": cache_reused,
                    "chunk_count": n_chunks,
                    "status": "ok",
                }
            )
            events.append(
                {
                    "event": "parser_chunking_complete",
                    "timestamp": datetime.now(UTC).isoformat(),
                    "article_ref": ref,
                    "article_key": key,
                    "source_kind": source_kind,
                    "text_source": str(source_path.relative_to(REPO_ROOT)),
                    "text_chars": text_chars,
                    "pdf_pages": pdf_pages,
                    "cache_reused": cache_reused,
                    "chunk_count": n_chunks,
                    "network_fetch_attempted": False,
                    "production_import_attempted": False,
                    "graph_import_allowed": False,
                    "ladybugdb_written": False,
                }
            )
            n_ok += 1
            print(f"  OK {ref}: chunks={n_chunks} ({source_kind})")
        except MetadataOnlySource as exc:
            skip_reason = str(exc)
            print(f"  SKIP {ref}: {skip_reason}")
            skip_reason_counts[skip_reason] += 1
            per_article.append(
                {
                    "article_ref": ref,
                    "article_key": key,
                    "source_kind": "metadata_only",
                    "status": "skipped_metadata_only",
                    "skip_reason": skip_reason,
                }
            )
            events.append(
                {
                    "event": "parser_chunking_skipped_metadata_only",
                    "timestamp": datetime.now(UTC).isoformat(),
                    "article_ref": ref,
                    "article_key": key,
                    "source_kind": "metadata_only",
                    "skip_reason": skip_reason,
                    "network_fetch_attempted": False,
                    "production_import_attempted": False,
                    "graph_import_allowed": False,
                    "ladybugdb_written": False,
                }
            )
            n_skipped += 1
        except Exception as exc:
            err = str(exc)[:200]
            print(f"  FAIL {ref}: {err}")
            per_article.append(
                {
                    "article_ref": ref,
                    "article_key": key,
                    "status": "error",
                    "error": err,
                }
            )
            events.append(
                {
                    "event": "parser_chunking_error",
                    "timestamp": datetime.now(UTC).isoformat(),
                    "article_ref": ref,
                    "article_key": key,
                    "error": err,
                    "network_fetch_attempted": False,
                    "production_import_attempted": False,
                    "graph_import_allowed": False,
                    "ladybugdb_written": False,
                }
            )
            n_err += 1

    with open(EVENTS_LOG, "w") as f:
        for event in events:
            f.write(json.dumps(event) + "\n")

    summary = {
        "schema_version": "r024-218-document-parser-chunking-summary.v00.01",
        "total": len(per_article),
        "ok": n_ok,
        "skipped": n_skipped,
        "errors": n_err,
        "source_kind_counts": dict(sorted(source_kind_counts.items())),
        "skip_reason_counts": dict(sorted(skip_reason_counts.items())),
        "chunk_count_min": min(chunk_counts) if chunk_counts else 0,
        "chunk_count_max": max(chunk_counts) if chunk_counts else 0,
        "chunk_count_total": sum(chunk_counts),
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "graph_import_allowed": False,
        "ladybugdb_written": False,
    }
    SUMMARY.write_text(json.dumps(summary, indent=2))
    print(f"summary: {n_ok} ok, {n_skipped} skipped, {n_err} errors")
    print(f"source kinds: {dict(sorted(source_kind_counts.items()))}")
    print(f"skip reasons: {dict(sorted(skip_reason_counts.items()))}")
    print(f"chunks total: {summary['chunk_count_total']}")
    return 0 if n_err == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
