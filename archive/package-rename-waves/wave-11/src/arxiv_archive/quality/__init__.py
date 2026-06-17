# Formerly: src/arxiv_archive/quality/__init__.py

"""Local non-blocking quality diagnostics."""

from arxiv_archive.quality.baselines import baseline_delta, baseline_from_summary, read_baseline
from arxiv_archive.quality.maintainability_report import (
    build_maintainability_report,
    maintainability_report_to_json,
    write_maintainability_report,
)
from arxiv_archive.quality.thresholds import (
    DEFAULT_THRESHOLDS,
    MaintainabilityThresholds,
    severity_for_score,
)

__all__ = [
    "DEFAULT_THRESHOLDS",
    "MaintainabilityThresholds",
    "baseline_delta",
    "baseline_from_summary",
    "build_maintainability_report",
    "maintainability_report_to_json",
    "read_baseline",
    "severity_for_score",
    "write_maintainability_report",
]
