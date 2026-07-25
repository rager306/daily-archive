# Formerly: src/arxiv_archive/quality/baselines.py

"""Baseline helpers for non-blocking maintainability diagnostics."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

BASELINE_SCHEMA_VERSION = "daily-archive-maintainability-baseline.v1"


def read_baseline(path: str | Path | None) -> dict[str, Any] | None:
    """Read a JSON maintainability baseline, returning None when omitted or absent."""
    if path is None:
        return None
    baseline_path = Path(path)
    if not baseline_path.exists():
        return None
    payload = json.loads(baseline_path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"maintainability baseline must be a JSON object: {baseline_path}")
    return payload


def baseline_from_summary(summary: dict[str, Any]) -> dict[str, Any]:
    """Build a compact baseline payload from a maintainability summary."""
    return {
        "schema_version": BASELINE_SCHEMA_VERSION,
        "max_score": float(summary.get("max_score", 0.0) or 0.0),
        "average_score": float(summary.get("average_score", 0.0) or 0.0),
        "total_functions": int(summary.get("total_functions", 0) or 0),
        "by_severity": dict(summary.get("by_severity", {})),
    }


def baseline_delta(summary: dict[str, Any], baseline: dict[str, Any] | None) -> dict[str, Any]:
    """Compare current diagnostic totals to an optional baseline without gating."""
    if baseline is None:
        return {
            "baseline_present": False,
            "max_score_delta": None,
            "average_score_delta": None,
            "function_count_delta": None,
            "severity_count_delta": {},
        }
    current_by_severity = (
        summary.get("by_severity", {}) if isinstance(summary.get("by_severity"), dict) else {}
    )
    baseline_by_severity = (
        baseline.get("by_severity", {}) if isinstance(baseline.get("by_severity"), dict) else {}
    )
    severity_keys = sorted(set(current_by_severity) | set(baseline_by_severity))
    return {
        "baseline_present": True,
        "max_score_delta": _float_delta(summary.get("max_score"), baseline.get("max_score")),
        "average_score_delta": _float_delta(
            summary.get("average_score"), baseline.get("average_score")
        ),
        "function_count_delta": int(summary.get("total_functions", 0) or 0)
        - int(baseline.get("total_functions", 0) or 0),
        "severity_count_delta": {
            key: int(current_by_severity.get(key, 0) or 0)
            - int(baseline_by_severity.get(key, 0) or 0)
            for key in severity_keys
        },
    }


def _float_delta(current: Any, previous: Any) -> float:
    return round(float(current or 0.0) - float(previous or 0.0), 4)


__all__ = ["BASELINE_SCHEMA_VERSION", "baseline_delta", "baseline_from_summary", "read_baseline"]
