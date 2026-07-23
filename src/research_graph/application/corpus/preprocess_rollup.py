"""Aggregate preprocess body diagnostics (M233/M235).

Pure rollup for operator visibility. Never authorizes import and never
drives handoff/proof verdicts (ADR-036 enrichment-only).

M235: ``empty_preprocess_rollup`` is the fail-closed default factory for
composition result dataclasses (fresh dict per call).
"""

from __future__ import annotations

from collections import Counter
from collections.abc import Mapping, Sequence
from typing import Any


def rollup_preprocess_bodies(
    rows: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    """Count quality_status and keyword_source across preprocess rows.

    Empty input returns zero counts. Always import-blocked and non-gating.
    """
    quality = Counter()
    keywords = Counter()
    for row in rows:
        q = row.get("quality_status")
        quality[str(q) if q is not None and str(q).strip() else "unknown"] += 1
        k = row.get("keyword_source")
        keywords[str(k) if k is not None and str(k).strip() else "unknown"] += 1
    return {
        "body_count": len(rows),
        "quality_status_counts": dict(sorted(quality.items())),
        "keyword_source_counts": dict(sorted(keywords.items())),
        "drives_verdict": False,
        "import_eligible": False,
    }


def empty_preprocess_rollup() -> dict[str, Any]:
    """Fresh empty rollup for dataclass default_factory (fail-closed)."""
    return rollup_preprocess_bodies([])


__all__ = ["empty_preprocess_rollup", "rollup_preprocess_bodies"]
