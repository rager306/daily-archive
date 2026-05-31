"""Canonical identifier helpers for redacted review/staging artifacts.

These helpers centralize deterministic IDs and hashes so candidate locator,
import-boundary, and future graph-staging modules share one identity surface
instead of embedding ID formats inside chunking or staging assembly code.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any


def canonical_paper_id(value: object) -> str:
    """Return a stable non-empty paper identifier string."""
    paper_id = str(value).strip()
    return paper_id or "unknown-paper"


def canonical_source_id(paper_id: str, role: str = "full-text") -> str:
    """Return the canonical source identity used by staging locators."""
    return f"source-{canonical_paper_id(paper_id)}-{role}"


def canonical_locator_id(*, paper_id: str, route_name: str, namespace: str = "m021", index: int = 1) -> str:
    """Return a deterministic candidate locator ID."""
    return f"{namespace}-{canonical_paper_id(paper_id)}-{route_name}-{index:03d}"


def canonical_import_candidate_id(*, method_id: str, refusal_reason: str, index: int) -> str:
    """Return a deterministic import-boundary candidate ID."""
    return f"{method_id}:{refusal_reason}:{index:06d}"


def canonical_package_id(*, method_id: str) -> str:
    """Return a deterministic synthetic package ID for aggregate benchmark candidates."""
    return f"benchmark-method:{method_id}"


def stable_json_hash(payload: dict[str, Any]) -> str:
    """Hash a JSON payload using stable key ordering and compact separators."""
    return hashlib.sha256(json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def stable_span_hash(*, source_id: str, source_hash: str, char_start: int, char_end: int, route_name: str) -> str:
    """Return a stable hash for a normalized Markdown character span."""
    return stable_json_hash(
        {
            "source_id": source_id,
            "source_hash": source_hash,
            "coordinate_space": "normalized_markdown_char",
            "char_start": char_start,
            "char_end": char_end,
            "route_name": route_name,
        }
    )


def artifact_record_hash(*, source_path: str | Path, route_name: str) -> str:
    """Return a stable hash for an artifact-level fallback span."""
    return hashlib.sha256(f"{source_path}:artifact_record:{route_name}".encode()).hexdigest()


__all__ = [
    "artifact_record_hash",
    "canonical_import_candidate_id",
    "canonical_locator_id",
    "canonical_package_id",
    "canonical_paper_id",
    "canonical_source_id",
    "stable_json_hash",
    "stable_span_hash",
]
