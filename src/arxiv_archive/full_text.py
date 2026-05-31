"""Compatibility delegates for full-text ingestion.

The implementation lives in :mod:`arxiv_archive.ingestion.loader` during the
loader-stack migration.  Keep this module as the legacy public import surface.
"""

from __future__ import annotations

from arxiv_archive.ingestion.loader import (
    ExtractionMode,
    FullTextIngestionResult,
    FullTextQualityReport,
    FullTextQualityStatus,
    FullTextSource,
    FullTextSourceType,
    assess_full_text_quality,
    full_text_source_for_paper,
    ingest_full_text,
)

__all__ = [
    "ExtractionMode",
    "FullTextIngestionResult",
    "FullTextQualityReport",
    "FullTextQualityStatus",
    "FullTextSource",
    "FullTextSourceType",
    "assess_full_text_quality",
    "full_text_source_for_paper",
    "ingest_full_text",
]
