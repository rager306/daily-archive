"""Compatibility delegates for article source loading.

The implementation lives in :mod:`arxiv_archive.ingestion.loader` and logging is
split into :mod:`arxiv_archive.ingestion.logging` during the loader-stack
migration.  Keep this module as the legacy public import surface.


Formerly: src/arxiv_archive/article_loader.py"""

from __future__ import annotations

from arxiv_archive.ingestion.loader import (
    ArticleLoadResult,
    ArticleLoadSource,
    ArticleOutcome,
    ArticleSourceMetadata,
    ArticleSourceType,
    classify_article_source,
    load_article_source,
)

__all__ = [
    "ArticleLoadResult",
    "ArticleLoadSource",
    "ArticleOutcome",
    "ArticleSourceMetadata",
    "ArticleSourceType",
    "classify_article_source",
    "load_article_source",
]
