# Formerly: src/arxiv_archive/quality/riskratchet_adapter.py

"""Adapter around riskratchet's Python API for informational diagnostics."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from research_graph.infrastructure.quality.thresholds import (
    DEFAULT_THRESHOLDS,
    MaintainabilityThresholds,
)

RISK_REPORT_SCHEMA_VERSION = "daily-archive-riskratchet-adapter.v1"


@dataclass(frozen=True)
class RiskratchetDiagnostic:
    """JSON-native diagnostic envelope from a riskratchet scan."""

    status: str
    report: dict[str, Any]
    error: str | None = None

    @property
    def available(self) -> bool:
        return self.status == "ok"


def run_riskratchet_scan(
    paths: tuple[Path, ...],
    *,
    root: Path | None = None,
    thresholds: MaintainabilityThresholds = DEFAULT_THRESHOLDS,
    include: tuple[str, ...] = (),
    exclude: tuple[str, ...] = (),
    allow: tuple[str, ...] = (),
) -> RiskratchetDiagnostic:
    """Run riskratchet and return a non-blocking JSON-native diagnostic.

    Import/runtime failures are captured in the envelope instead of raised so the
    caller can surface diagnostic unavailability without changing pass/fail state.
    """
    try:
        from riskratchet import analyze  # type: ignore[import-not-found]
    except ImportError as exc:
        return RiskratchetDiagnostic(
            status="unavailable", report=_empty_report(paths, thresholds), error=str(exc)
        )

    try:
        risk_report = analyze(
            paths,
            root=root or Path.cwd(),
            include=include,
            exclude=exclude,
            allow=allow,
            use_git=False,
            # pyrefly: ignore [bad-argument-type]
            missing_coverage_policy="ignore",  # ty:ignore[invalid-argument-type]
        )
    except Exception as exc:  # pragma: no cover - exact third-party failures vary
        return RiskratchetDiagnostic(
            status="error", report=_empty_report(paths, thresholds), error=str(exc)
        )

    return RiskratchetDiagnostic(
        status="ok", report=_serialize_report(risk_report, paths, thresholds)
    )


def _serialize_report(
    report: Any, paths: tuple[Path, ...], thresholds: MaintainabilityThresholds
) -> dict[str, Any]:
    functions = [
        _serialize_function(function, thresholds) for function in getattr(report, "functions", ())
    ]
    by_severity = {"low": 0, "medium": 0, "high": 0, "critical": 0}
    for function in functions:
        by_severity[function["severity"]] += 1
    scores = [float(function["score"]) for function in functions]
    max_score = max(scores, default=0.0)
    average_score = round(sum(scores) / len(scores), 4) if scores else 0.0
    return {
        "schema_version": RISK_REPORT_SCHEMA_VERSION,
        "tool": "riskratchet",
        "diagnostic_only": True,
        "blocking": False,
        "scope": [str(path) for path in paths],
        "thresholds": thresholds.as_dict(),
        "summary": {
            "total_functions": int(
                getattr(report, "analyzed_functions", len(functions)) or len(functions)
            ),
            "emitted_functions": len(functions),
            "total_files": len(getattr(report, "files", ()) or ()),
            "coverage_status": getattr(report, "coverage_status", None),
            "max_score": round(max_score, 4),
            "average_score": average_score,
            "by_severity": by_severity,
        },
        "functions": sorted(
            functions, key=lambda item: (-float(item["score"]), item["path"], item["qualname"])
        ),
    }


def _serialize_function(function: Any, thresholds: MaintainabilityThresholds) -> dict[str, Any]:
    score = float(getattr(function, "score", 0.0) or 0.0)
    function_id = function.id
    span = getattr(function, "span", None)
    complexity = getattr(function, "complexity", None)
    coverage = getattr(function, "coverage", None)
    churn = getattr(function, "churn", None)
    return {
        "path": str(getattr(function_id, "path", "")),
        "qualname": str(getattr(function_id, "qualname", "")),
        "score": round(score, 4),
        "severity": thresholds.severity_for_score(score),
        "start_line": getattr(span, "start_line", None),
        "end_line": getattr(span, "end_line", None),
        "complexity": getattr(complexity, "cyclomatic", None),
        "line_coverage": getattr(coverage, "line_coverage", None),
        "churn_commits": getattr(churn, "commits", None),
        "is_public": bool(getattr(function, "is_public", False)),
    }


def _empty_report(paths: tuple[Path, ...], thresholds: MaintainabilityThresholds) -> dict[str, Any]:
    return {
        "schema_version": RISK_REPORT_SCHEMA_VERSION,
        "tool": "riskratchet",
        "diagnostic_only": True,
        "blocking": False,
        "scope": [str(path) for path in paths],
        "thresholds": thresholds.as_dict(),
        "summary": {
            "total_functions": 0,
            "emitted_functions": 0,
            "total_files": 0,
            "coverage_status": "unavailable",
            "max_score": 0.0,
            "average_score": 0.0,
            "by_severity": {"low": 0, "medium": 0, "high": 0, "critical": 0},
        },
        "functions": [],
    }


def report_to_json(report: dict[str, Any]) -> str:
    """Serialize a report deterministically for artifacts and CLI output."""
    return json.dumps(report, indent=2, sort_keys=True) + "\n"


__all__ = [
    "RISK_REPORT_SCHEMA_VERSION",
    "RiskratchetDiagnostic",
    "report_to_json",
    "run_riskratchet_scan",
]
