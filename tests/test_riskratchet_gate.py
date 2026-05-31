"""Tests for the non-blocking riskratchet maintainability diagnostic."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

from arxiv_archive.quality import build_maintainability_report, write_maintainability_report
from arxiv_archive.quality.baselines import baseline_delta, read_baseline
from arxiv_archive.quality.thresholds import MaintainabilityThresholds


def test_thresholds_classify_boundary_scores() -> None:
    thresholds = MaintainabilityThresholds(medium=10.0, high=20.0, critical=30.0)

    assert thresholds.severity_for_score(0.0) == "low"
    assert thresholds.severity_for_score(10.0) == "medium"
    assert thresholds.severity_for_score(20.0) == "high"
    assert thresholds.severity_for_score(30.0) == "critical"


def test_maintainability_report_is_diagnostic_only_for_real_source_file() -> None:
    report = build_maintainability_report(paths=["src/arxiv_archive/validation_logging.py"])

    assert report["status"] == "diagnostic_complete"
    assert report["diagnostic_only"] is True
    assert report["blocking"] is False
    assert report["pass_fail_affected"] is False
    assert report["summary"]["total_functions"] > 0
    assert report["summary"]["max_score"] > 0
    assert report["baseline_delta"]["baseline_present"] is False
    assert report["riskratchet"]["blocking"] is False
    assert report["riskratchet"]["functions"]


def test_baseline_delta_reports_score_and_severity_changes() -> None:
    summary = {
        "max_score": 40.0,
        "average_score": 20.0,
        "total_functions": 3,
        "by_severity": {"low": 1, "medium": 2, "high": 0, "critical": 0},
    }
    baseline = {
        "max_score": 35.0,
        "average_score": 25.0,
        "total_functions": 2,
        "by_severity": {"low": 2, "medium": 0, "high": 0, "critical": 0},
    }

    delta = baseline_delta(summary, baseline)

    assert delta == {
        "baseline_present": True,
        "max_score_delta": 5.0,
        "average_score_delta": -5.0,
        "function_count_delta": 1,
        "severity_count_delta": {"critical": 0, "high": 0, "low": -1, "medium": 2},
    }


def test_malformed_baseline_bubbles_as_value_error(tmp_path: Path) -> None:
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text("[]\n", encoding="utf-8")

    with pytest.raises(ValueError, match="baseline must be a JSON object"):
        read_baseline(baseline_path)


def test_cli_writes_json_report_without_blocking(tmp_path: Path) -> None:
    output_path = tmp_path / "maintainability.json"
    result = subprocess.run(
        [
            "uv",
            "run",
            "python",
            "-m",
            "arxiv_archive",
            "quality",
            "maintainability",
            "src/arxiv_archive/validation_logging.py",
            "--output",
            str(output_path),
            "--json",
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    stdout_payload = json.loads(result.stdout)
    file_payload = json.loads(output_path.read_text(encoding="utf-8"))
    assert stdout_payload["status"] == "diagnostic_complete"
    assert stdout_payload["blocking"] is False
    assert stdout_payload["output_path"] == str(output_path)
    assert file_payload["blocking"] is False
    assert file_payload["summary"]["total_functions"] > 0


def test_write_report_creates_parent_directories(tmp_path: Path) -> None:
    report = {"schema_version": "test", "blocking": False}
    output_path = tmp_path / "nested" / "report.json"

    written = write_maintainability_report(report, output_path)

    assert written == output_path
    assert json.loads(output_path.read_text(encoding="utf-8")) == report
