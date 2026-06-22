#!/usr/bin/env python3
"""R024 S02: parser+chunking replay on 53-article corpus.

Reads selection.json. For source_kind=html_native uses article.html via FullTextSource.
For source_kind=pdf_converted uses pdf-text-cache/<key>.txt via FullTextSource.

Fail-closed invariants preserved.
"""

from __future__ import annotations

import json
import sys
from datetime import UTC, datetime
from pathlib import Path

REPO_ROOT = Path("/root/daily-archive")
SELECTION = REPO_ROOT / "data" / "r024-53-document-corpus-v1" / "selection.json"
OUTPUT_DIR = REPO_ROOT / "data" / "r024-53-document-corpus-v1" / "parser-chunking"
CACHE_DIR = REPO_ROOT / "data" / "r024-53-document-corpus-v1" / "pdf-text-cache"
EVENTS_LOG = OUTPUT_DIR / "events.jsonl"
SUMMARY = OUTPUT_DIR / "summary.json"

from research_graph.infrastructure.corpus.ingestion import FullTextSource, ingest_full_text
from research_graph.infrastructure.corpus.parsing.parser import parse_article
from research_graph.infrastructure.papers.indexing.parsed_page_index import (
    build_page_index_from_parsed,
)


def find_html_source(article_dir: Path) -> Path | None:
    """Find text/markdown/html source file."""
    for ext in ("*.md", "*.markdown", "*.html", "*.txt"):
        matches = list(article_dir.rglob(ext))
        if matches:
            for pref in ("abs.html", "article.html"):
                for m in matches:
                    if m.name == pref:
                        return m
            return matches[0]
    return None


def main() -> int:
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    sel = json.loads(SELECTION.read_text())
    articles = sel["articles"]
    print(f"Processing {len(articles)} articles...")

    per_article: list[dict[str, object]] = []
    events: list[dict[str, object]] = []
    n_ok = 0
    n_err = 0
    for a in articles:
        ref = a["article_ref"]
        key = a["article_key"]
        source_kind = a.get("source_kind", "html_native")

        try:
            if source_kind == "pdf_converted":
                text_path = CACHE_DIR / f"{key}.txt"
                if not text_path.exists():
                    raise FileNotFoundError(f"PDF text cache missing: {text_path}")
                source_path = text_path
            else:
                article_dirs = list(REPO_ROOT.glob(f"data/article_catalog/article_catalog/{ref}"))
                if not article_dirs:
                    raise FileNotFoundError(f"No article dir for {ref}")
                src = find_html_source(article_dirs[0])
                if not src:
                    raise FileNotFoundError(f"No text source in {article_dirs[0]}")
                source_path = src

            source = FullTextSource(
                paper_id=ref,
                source_type="text",
                source_path=source_path,
            )
            ingestion = ingest_full_text(source)
            parsed = parse_article(ingestion)
            page_index = build_page_index_from_parsed(parsed)
            n_chunks = len(page_index.nodes) if hasattr(page_index, "nodes") else 0
            per_article.append(
                {
                    "article_ref": ref,
                    "article_key": key,
                    "source_kind": source_kind,
                    "text_source": str(source_path.relative_to(REPO_ROOT)),
                    "chunk_count": n_chunks,
                    "status": "ok",
                }
            )
            events.append(
                {
                    "event": "parser_chunking_complete",
                    "timestamp": datetime.now(UTC).isoformat(),
                    "article_ref": ref,
                    "source_kind": source_kind,
                    "chunk_count": n_chunks,
                    "network_fetch_attempted": False,
                    "production_import_attempted": False,
                    "graph_import_allowed": False,
                    "ladybugdb_written": False,
                }
            )
            n_ok += 1
            print(f"  OK {ref}: chunks={n_chunks} ({source_kind})")
        except Exception as e:
            err = str(e)[:120]
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
                    "error": err,
                }
            )
            n_err += 1

    EVENTS_LOG.parent.mkdir(parents=True, exist_ok=True)
    with open(EVENTS_LOG, "w") as f:
        for e in events:
            f.write(json.dumps(e) + "\n")

    summary = {
        "schema_version": "r024-53-document-parser-chunking-summary.v00.01",
        "total": len(per_article),
        "ok": n_ok,
        "errors": n_err,
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "graph_import_allowed": False,
        "ladybugdb_written": False,
    }
    SUMMARY.write_text(json.dumps(summary, indent=2))
    print(f"summary: {n_ok} ok, {n_err} errors")
    return 0 if n_err == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
