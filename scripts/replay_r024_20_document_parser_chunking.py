#!/usr/bin/env python3
"""R024 S02: parser+chunking replay on 20-article corpus (M116 framework).

Runs parse_article + build_page_index_from_parsed on 20 articles from
data/r024-20-document-corpus-v1/selection.json. Captures per-article artifacts.

Fail-closed invariants:
- network_fetch_attempted=false
- production_import_attempted=false
- graph_import_allowed=false
- ladybugdb_written=false

Outputs:
- data/r024-20-document-corpus-v1/parser-chunking/events.jsonl
- data/r024-20-document-corpus-v1/parser-chunking/summary.json
"""

from __future__ import annotations

import json
import sys
from datetime import UTC, datetime
from pathlib import Path

REPO_ROOT = Path("/root/daily-archive")
SELECTION = REPO_ROOT / "data" / "r024-20-document-corpus-v1" / "selection.json"
OUTPUT_DIR = REPO_ROOT / "data" / "r024-20-document-corpus-v1" / "parser-chunking"
EVENTS_LOG = OUTPUT_DIR / "events.jsonl"
SUMMARY = OUTPUT_DIR / "summary.json"

from research_graph.infrastructure.corpus.ingestion import FullTextSource, ingest_full_text
from research_graph.infrastructure.corpus.parsing.parser import parse_article
from research_graph.infrastructure.papers.indexing.parsed_page_index import (
    build_page_index_from_parsed,
)


def find_text_source(article_dir: Path) -> Path | None:
    """Find text/markdown/html source file (recursive). Prefer abs.html, article.html."""
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
    sel = json.loads(SELECTION.read_text())
    articles = sel["articles"]
    print(f"Processing {len(articles)} articles...")

    per_article: list[dict[str, str | int | None]] = []
    events: list[dict[str, object]] = []
    for a in articles:
        ref = a["article_ref"]
        key = a["article_key"]
        article_dirs = list(REPO_ROOT.glob(f"data/article_catalog/article_catalog/{ref}"))
        if not article_dirs:
            print(f"  SKIP {ref}: no article dir")
            continue
        article_dir = article_dirs[0]
        text_source = find_text_source(article_dir)
        if not text_source:
            print(f"  SKIP {ref}: no text source in {article_dir}")
            continue
        try:
            source = FullTextSource(paper_id=ref, source_type="text", source_path=text_source)
            ingestion = ingest_full_text(source)
            parsed = parse_article(ingestion)
            page_index = build_page_index_from_parsed(parsed)
            n_chunks = len(page_index.nodes) if hasattr(page_index, "nodes") else 0
            per_article.append(
                {
                    "article_ref": ref,
                    "article_key": key,
                    "text_source": str(text_source.relative_to(REPO_ROOT)),
                    "chunk_count": n_chunks,
                    "status": "ok",
                }
            )
            events.append(
                {
                    "event": "parser_chunking_complete",
                    "timestamp": datetime.now(UTC).isoformat(),
                    "article_ref": ref,
                    "chunk_count": n_chunks,
                    "text_source": str(text_source.relative_to(REPO_ROOT)),
                    "network_fetch_attempted": False,
                    "production_import_attempted": False,
                    "graph_import_allowed": False,
                    "ladybugdb_written": False,
                }
            )
            print(f"  OK {ref}: chunks={n_chunks}")
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

    EVENTS_LOG.parent.mkdir(parents=True, exist_ok=True)
    with open(EVENTS_LOG, "w") as f:
        for e in events:
            f.write(json.dumps(e) + "\n")
    n_ok = sum(1 for a in per_article if a.get("status") == "ok")
    n_err = sum(1 for a in per_article if a.get("status") == "error")
    summary = {
        "schema_version": "r024-20-document-parser-chunking-summary.v00.01",
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
