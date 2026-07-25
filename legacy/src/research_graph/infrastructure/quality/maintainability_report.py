# Formerly: src/arxiv_archive/quality/maintainability_report.py

"""Maintainability diagnostic report assembly."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from research_graph.infrastructure.quality.baselines import (
    baseline_delta,
    baseline_from_summary,
    read_baseline,
)
from research_graph.infrastructure.quality.riskratchet_adapter import (
    report_to_json,
    run_riskratchet_scan,
)
from research_graph.infrastructure.quality.scopes import (
    DEFAULT_QUALITY_EXCLUDES,
    normalize_scope,
    scope_payload,
)
from research_graph.infrastructure.quality.thresholds import (
    DEFAULT_THRESHOLDS,
    MaintainabilityThresholds,
)

MAINTAINABILITY_REPORT_SCHEMA_VERSION = "daily-archive-maintainability-report.v1"


def build_maintainability_report(
    *,
    paths: tuple[str | Path, ...] | list[str | Path] | None = None,
    root: str | Path | None = None,
    baseline_path: str | Path | None = None,
    thresholds: MaintainabilityThresholds = DEFAULT_THRESHOLDS,
) -> dict[str, Any]:
    """Build a non-blocking maintainability diagnostic report."""
    scope_paths = normalize_scope(paths)
    diagnostic = run_riskratchet_scan(
        scope_paths,
        root=Path(root) if root is not None else None,
        thresholds=thresholds,
        exclude=DEFAULT_QUALITY_EXCLUDES,
    )
    risk_report = diagnostic.report
    summary = dict(risk_report.get("summary", {}))
    baseline = read_baseline(baseline_path)
    report = {
        "schema_version": MAINTAINABILITY_REPORT_SCHEMA_VERSION,
        "status": "diagnostic_complete" if diagnostic.available else "diagnostic_unavailable",
        "diagnostic_only": True,
        "blocking": False,
        "pass_fail_affected": False,
        "tool_status": diagnostic.status,
        "tool_error": diagnostic.error,
        "scope": scope_payload(scope_paths),
        "summary": summary,
        "baseline_delta": baseline_delta(summary, baseline),
        "suggested_baseline": baseline_from_summary(summary),
        "riskratchet": risk_report,
    }
    return report


def write_maintainability_report(report: dict[str, Any], output_path: str | Path) -> Path:
    """Write a maintainability report JSON artifact."""
    path = Path(output_path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return path


def maintainability_report_to_json(report: dict[str, Any]) -> str:
    """Serialize a maintainability report deterministically."""
    return report_to_json(report)


__all__ = [
    "MAINTAINABILITY_REPORT_SCHEMA_VERSION",
    "build_maintainability_report",
    "maintainability_report_to_json",
    "write_maintainability_report",
]
