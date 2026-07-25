"""Application helpers for article catalog selection shapes."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

SELECTION_SCHEMA_VERSION = "article-corpus-selection.v00.01"
CATALOG_SCHEMA_VERSION = "article-catalog.v00.01"
ARTICLE_SCHEMA_VERSION = "article.v00.01"
CURRENT_INDEX_SELECTION_ID = "current-article-catalog-index"


def build_current_catalog_index_selection(index_path: Path) -> dict[str, Any]:
    """Build a no-network selection payload from the canonical article index."""
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
        "schema_version": SELECTION_SCHEMA_VERSION,
        "selection_id": CURRENT_INDEX_SELECTION_ID,
        "catalog_schema_version": CATALOG_SCHEMA_VERSION,
        "article_schema_version": ARTICLE_SCHEMA_VERSION,
        "network_policy": {
            "test_phase_must_not_fetch": True,
            "pipeline_phase_reads_catalog_only": True,
        },
        "articles": articles,
    }
