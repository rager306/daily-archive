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

# ruff: noqa: I001

import json
import sys
import tempfile
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from research_graph.application.corpus.article_catalog_selection import (
    build_current_catalog_index_selection,
)

# pyrefly: ignore [missing-import]
from verify_m025_article_catalog import main

ROOT = SCRIPT_DIR.parents[0]
DEFAULT_CATALOG = ROOT / "data" / "article_catalog" / "catalog.json"
DEFAULT_INDEX = ROOT / "data" / "article_catalog" / "article_catalog" / "index.json"


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
        selection_path.write_text(
            json.dumps(build_current_catalog_index_selection(DEFAULT_INDEX), indent=2) + "\n",
            encoding="utf-8",
        )
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
