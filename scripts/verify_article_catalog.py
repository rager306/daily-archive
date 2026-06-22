#!/usr/bin/env python3
"""Generic article catalog verifier entrypoint.

This delegates to the M025-origin verifier core without imposing an M025
selection_id, so milestone-specific corpus selections can reuse the same
index-only, no-network, path-safe validation behavior.

When invoked without arguments, the wrapper validates the current canonical
catalog/index pair using a temporary index-derived selection. That keeps the
public command ``uv run python scripts/verify_article_catalog.py`` useful while
preserving the core verifier's explicit-argument contract for specialized runs.
"""

from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from verify_m025_article_catalog import main

ROOT = SCRIPT_DIR.parents[0]
DEFAULT_CATALOG = ROOT / "data" / "article_catalog" / "catalog.json"
DEFAULT_INDEX = ROOT / "data" / "article_catalog" / "article_catalog" / "index.json"


def build_default_selection(index_path: Path) -> dict[str, Any]:
    index = json.loads(index_path.read_text(encoding="utf-8"))
    articles = [
        {
            "article_ref": row["article_ref"],
            "source_code": row["source_code"],
            "title": row.get("title"),
        }
        for row in index.get("articles", [])
        if isinstance(row, dict) and row.get("article_ref") and row.get("source_code")
    ]
    return {
        "schema_version": "article-corpus-selection.v00.01",
        "selection_id": "current-article-catalog-index",
        "catalog_schema_version": "article-catalog.v00.01",
        "article_schema_version": "article.v00.01",
        "network_policy": {
            "test_phase_must_not_fetch": True,
            "pipeline_phase_reads_catalog_only": True,
        },
        "articles": articles,
    }


def run_core(effective_argv: list[str]) -> int:
    return main(
        effective_argv,
        default_expected_selection_id=None,
        label="article catalog",
        default_report_title="Article Catalog Readiness Report",
    )


def run(argv: list[str]) -> int:
    if len(argv) != 1:
        return run_core(argv)

    with tempfile.TemporaryDirectory(prefix="article-catalog-selection-") as tmp_dir:
        selection_path = Path(tmp_dir) / "selection.json"
        selection_path.write_text(json.dumps(build_default_selection(DEFAULT_INDEX), indent=2) + "\n", encoding="utf-8")
        return run_core(
            [
                argv[0],
                "--catalog",
                str(DEFAULT_CATALOG),
                "--index",
                str(DEFAULT_INDEX),
                "--selection",
                str(selection_path),
                "--validate-only",
                "--require-index",
                "--check-index-titles",
            ]
        )


if __name__ == "__main__":
    raise SystemExit(run(sys.argv))
