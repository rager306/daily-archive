"""Compatibility shim for article artifact reducer.

The canonical import path is now ``arxiv_archive.artifacts.reducer``.
This module remains to preserve imports created before M082.
"""

from arxiv_archive.artifacts.reducer import (
    REDUCER_SCHEMA_VERSION,
    _safety_defaults,
    merge_article_artifact_results,
    aggregate_article_artifact_log,
    DEFAULT_VALIDATION_BUCKETS,
)

__all__ = [
    'REDUCER_SCHEMA_VERSION',
    '_safety_defaults',
    'merge_article_artifact_results',
    'aggregate_article_artifact_log',
    'DEFAULT_VALIDATION_BUCKETS',
]
