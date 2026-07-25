#!/usr/bin/env python3
"""R024 parser+chunking replay wrapper for the 20-article corpus."""

from __future__ import annotations

import sys
from pathlib import Path

from research_graph.application.corpus.parser_replay import ParserReplayUseCase
from research_graph.infrastructure.corpus.parsing.replay_adapters import (
    ExistingFullTextParserAdapter,
    FilesystemParserReplaySourceLoader,
    PageIndexChunkWriterAdapter,
    ParserReplayArtifactWriter,
    SelectionJsonArticleSelector,
)

REPO_ROOT = Path(__file__).resolve().parents[1]
SELECTION = REPO_ROOT / "data" / "r024-20-document-corpus-v1" / "selection.json"
OUTPUT_DIR = REPO_ROOT / "data" / "r024-20-document-corpus-v1" / "parser-chunking"
CACHE_DIR = REPO_ROOT / "data" / "r024-20-document-corpus-v1" / "pdf-text-cache"
EVENTS_LOG = OUTPUT_DIR / "events.jsonl"
SUMMARY = OUTPUT_DIR / "summary.json"
CATALOG_PARENT = REPO_ROOT / "data" / "article_catalog"


def main() -> int:
    result = ParserReplayUseCase(
        article_selector=SelectionJsonArticleSelector(SELECTION),
        source_loader=FilesystemParserReplaySourceLoader(
            catalog_parent=CATALOG_PARENT,
            cache_dir=CACHE_DIR,
            repo_root=REPO_ROOT,
            prefer_pdf=False,
        ),
        full_text_parser=ExistingFullTextParserAdapter(),
        chunk_writer=PageIndexChunkWriterAdapter(),
    ).run()

    ParserReplayArtifactWriter(
        output_dir=OUTPUT_DIR,
        events_log=EVENTS_LOG,
        summary_path=SUMMARY,
        schema_version="r024-20-document-parser-chunking-summary.v00.01",
        repo_root=REPO_ROOT,
    ).write(result)

    print(
        f"summary: {result.completed_count} ok, {result.skipped_count} skipped, "
        f"{result.low_quality_count} low_quality, {result.failed_count} errors"
    )
    return 0 if result.failed_count == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
