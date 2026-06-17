# Formerly: src/arxiv_archive/identity/__init__.py

"""Canonical identity and deduplication helpers for arxiv_archive."""

from arxiv_archive.identity.canonicalization import (
    artifact_record_hash,
    canonical_import_candidate_id,
    canonical_locator_id,
    canonical_package_id,
    canonical_paper_id,
    canonical_source_id,
    stable_json_hash,
    stable_span_hash,
)
from arxiv_archive.identity.dedup import annotate_overlapping_signal_windows, append_unique, ranges_overlap

__all__ = [
    "annotate_overlapping_signal_windows",
    "append_unique",
    "artifact_record_hash",
    "canonical_import_candidate_id",
    "canonical_locator_id",
    "canonical_package_id",
    "canonical_paper_id",
    "canonical_source_id",
    "ranges_overlap",
    "stable_json_hash",
    "stable_span_hash",
]
