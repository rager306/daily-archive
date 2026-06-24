"""Tests for the non-blocking riskratchet maintainability diagnostic."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

from research_graph.infrastructure.quality import (
    build_maintainability_report,
    write_maintainability_report,
)
from research_graph.infrastructure.quality.baselines import baseline_delta, read_baseline
from research_graph.infrastructure.quality.thresholds import MaintainabilityThresholds

# pyrefly: ignore [missing-import]
from scripts import run_quality_gate as quality_gate_runner


def test_thresholds_classify_boundary_scores() -> None:
    thresholds = MaintainabilityThresholds(medium=10.0, high=20.0, critical=30.0)

    assert thresholds.severity_for_score(0.0) == "low"
    assert thresholds.severity_for_score(10.0) == "medium"
    assert thresholds.severity_for_score(20.0) == "high"
    assert thresholds.severity_for_score(30.0) == "critical"


def test_maintainability_report_is_diagnostic_only_for_real_source_file() -> None:
    report = build_maintainability_report(
        paths=["src/research_graph/workflows/validation/logging.py"]
    )

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
            "research_graph",
            "quality",
            "maintainability",
            "src/research_graph/workflows/validation/logging.py",
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


def test_quality_gate_runner_writes_json_and_human_reports(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    def fake_report(*, paths, baseline_path=None):
        return {
            "schema_version": "test-report",
            "status": "diagnostic_complete",
            "diagnostic_only": True,
            "blocking": False,
            "pass_fail_affected": False,
            "tool_status": "ok",
            "tool_error": None,
            "summary": {
                "total_functions": 2,
                "max_score": 12.5,
                "average_score": 6.25,
                "by_severity": {"low": 1, "medium": 1, "high": 0, "critical": 0},
            },
            "baseline_delta": {
                "baseline_present": False,
                "max_score_delta": None,
                "average_score_delta": None,
                "function_count_delta": None,
                "severity_count_delta": {},
            },
            "riskratchet": {"blocking": False, "functions": []},
        }

    monkeypatch.setattr(quality_gate_runner, "build_maintainability_report", fake_report)

    report = quality_gate_runner.run_quality_gate(
        paths=["src/research_graph/infrastructure/quality/baselines.py"],
        output_dir=tmp_path,
    )

    json_path = tmp_path / quality_gate_runner.JSON_REPORT_NAME
    human_path = tmp_path / quality_gate_runner.HUMAN_REPORT_NAME
    json_payload = json.loads(json_path.read_text(encoding="utf-8"))
    human_report = human_path.read_text(encoding="utf-8")

    assert report["blocking"] is False
    assert report["pass_fail_affected"] is False
    assert json_payload["quality_gate"]["diagnostic_only"] is True
    assert json_payload["quality_gate"]["touched_modules"] == [
        "src/research_graph/infrastructure/quality/baselines.py"
    ]
    assert json_payload["output_paths"] == {"json": str(json_path), "human": str(human_path)}
    assert "Diagnostic-only" in human_report
    assert "non-blocking" in human_report
    assert "Severity bands" in human_report


def test_quality_gate_touched_module_discovery_filters_to_source_and_scripts(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    completed = subprocess.CompletedProcess(
        args=["git"],
        returncode=0,
        stdout="\n".join(
            [
                "src/research_graph/infrastructure/quality/baselines.py",
                "scripts/run_quality_gate.py",
                "tests/test_riskratchet_gate.py",
                "README.md",
            ]
        ),
        stderr="",
    )
    monkeypatch.setattr(quality_gate_runner.subprocess, "run", lambda *args, **kwargs: completed)

    touched = quality_gate_runner.gather_touched_python_modules(base_ref="HEAD~1")

    assert touched == (
        Path("src/research_graph/infrastructure/quality/baselines.py"),
        Path("scripts/run_quality_gate.py"),
    )


def test_quality_gate_default_diagnostic_scope_paths_exist() -> None:
    assert quality_gate_runner.DEFAULT_DIAGNOSTIC_SCOPE
    for path in quality_gate_runner.DEFAULT_DIAGNOSTIC_SCOPE:
        assert path.exists(), path


def test_quality_gate_touched_module_discovery_falls_back_when_git_fails(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def raise_os_error(*args, **kwargs):
        raise OSError("git unavailable")

    monkeypatch.setattr(quality_gate_runner.subprocess, "run", raise_os_error)

    touched = quality_gate_runner.gather_touched_python_modules()

    assert touched == quality_gate_runner.DEFAULT_DIAGNOSTIC_SCOPE


def test_quality_gate_runner_always_zero_mode_reports_unavailable_without_blocking(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    def raise_runtime_error(*args, **kwargs):
        raise RuntimeError("riskratchet exploded")

    monkeypatch.setattr(quality_gate_runner, "run_quality_gate", raise_runtime_error)

    exit_code = quality_gate_runner.main(["--always-zero"])

    captured = capsys.readouterr()
    assert exit_code == 0
    assert "riskratchet: maintainability telemetry" in captured.out
    assert "status: diagnostic_unavailable" in captured.out
    assert "blocking: false" in captured.out
    assert "pass/fail affected: false" in captured.out


def test_quality_gate_runner_is_non_blocking_for_critical_scores(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    def critical_report(*, paths, baseline_path=None):
        return {
            "schema_version": "test-report",
            "status": "diagnostic_complete",
            "diagnostic_only": True,
            "blocking": False,
            "pass_fail_affected": False,
            "tool_status": "ok",
            "tool_error": None,
            "summary": {
                "total_functions": 1,
                "max_score": 99.0,
                "average_score": 99.0,
                "by_severity": {"low": 0, "medium": 0, "high": 0, "critical": 1},
            },
            "baseline_delta": {
                "baseline_present": True,
                "max_score_delta": 50.0,
                "average_score_delta": 50.0,
                "function_count_delta": 0,
                "severity_count_delta": {"critical": 1},
            },
            "riskratchet": {"blocking": False, "functions": [{"qualname": "risky", "score": 99.0}]},
        }

    monkeypatch.setattr(quality_gate_runner, "build_maintainability_report", critical_report)

    exit_code = quality_gate_runner.main(
        [
            "src/research_graph/infrastructure/quality/riskratchet_adapter.py",
            "--output-dir",
            str(tmp_path),
        ]
    )

    payload = json.loads(
        (tmp_path / quality_gate_runner.JSON_REPORT_NAME).read_text(encoding="utf-8")
    )
    assert exit_code == 0
    assert payload["blocking"] is False
    assert payload["quality_gate"]["blocking"] is False
    assert payload["summary"]["by_severity"]["critical"] == 1
