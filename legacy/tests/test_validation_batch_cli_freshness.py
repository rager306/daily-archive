from __future__ import annotations

import json
import subprocess
from datetime import UTC, datetime, timedelta
from pathlib import Path

from research_graph.workflows.validation.batch_provenance import (
    append_validation_cli_provenance,
    build_validation_cli_provenance_entry,
)


def _run_cli(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["uv", "run", "python", "-m", "arxiv_archive", *args],
        check=False,
        capture_output=True,
        text=True,
    )


def _provenance_fixture(tmp_path: Path) -> tuple[Path, Path, Path, dict]:
    input_path = tmp_path / "batch-state.json"
    output_path = tmp_path / "validation-scan-summary.json"
    input_path.write_text('{"state":"ready","raw":"RAW_FIXTURE_SENTINEL"}\n', encoding="utf-8")
    output_path.write_text('{"summary":"fresh"}\n', encoding="utf-8")
    started = datetime(2026, 5, 20, 6, 0, 0, tzinfo=UTC)
    entry = build_validation_cli_provenance_entry(
        command="validation-batch scan",
        argv=[
            "validation-batch",
            "scan",
            "--state-path",
            str(input_path),
            "--token",
            "secret-token",
        ],
        batch_id="fixture-freshness",
        input_paths=[input_path],
        output_paths=[output_path],
        status="scanned",
        started_at=started,
        completed_at=started + timedelta(seconds=1),
        exit_code=0,
        cwd=tmp_path,
        run_id="run-freshness",
        real_scan_performed=True,
    )
    log_path = append_validation_cli_provenance(tmp_path / "cli-run-log.jsonl", entry)
    return log_path, input_path, output_path, entry


def test_verify_artifacts_passes_for_fresh_outputs(tmp_path: Path) -> None:
    log_path, _input_path, _output_path, _entry = _provenance_fixture(tmp_path)

    result = _run_cli(
        "validation-batch",
        "verify-artifacts",
        "--provenance-log",
        str(log_path),
        "--run-id",
        "run-freshness",
        "--json",
    )

    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    assert payload["verdict"] == "fresh"
    assert payload["missing_count"] == 0
    assert payload["mismatch_count"] == 0
    assert "RAW_FIXTURE_SENTINEL" not in result.stdout
    assert "secret-token" not in result.stdout


def test_verify_artifacts_writes_report_path(tmp_path: Path) -> None:
    log_path, _input_path, _output_path, _entry = _provenance_fixture(tmp_path)
    report_path = tmp_path / "freshness-report.json"

    result = _run_cli(
        "validation-batch",
        "verify-artifacts",
        "--provenance-log",
        str(log_path),
        "--batch-id",
        "fixture-freshness",
        "--command",
        "validation-batch scan",
        "--report-path",
        str(report_path),
        "--json",
    )

    assert result.returncode == 0, result.stderr
    report = json.loads(report_path.read_text(encoding="utf-8"))
    assert report["schema_version"] == "m009-artifact-freshness-report.v1"
    assert report["verdict"] == "fresh"
    assert json.loads(result.stdout)["report_path"] == str(report_path)


def test_verify_artifacts_fails_after_output_mutation(tmp_path: Path) -> None:
    log_path, _input_path, output_path, _entry = _provenance_fixture(tmp_path)
    output_path.write_text('{"summary":"mutated"}\n', encoding="utf-8")

    result = _run_cli(
        "validation-batch",
        "verify-artifacts",
        "--provenance-log",
        str(log_path),
        "--run-id",
        "run-freshness",
        "--json",
    )

    assert result.returncode == 1
    payload = json.loads(result.stdout)
    assert payload["verdict"] == "stale"
    assert "output_hash_changed" in {diagnostic["code"] for diagnostic in payload["diagnostics"]}


def test_verify_artifacts_fails_after_output_deletion(tmp_path: Path) -> None:
    log_path, _input_path, output_path, _entry = _provenance_fixture(tmp_path)
    output_path.unlink()

    result = _run_cli(
        "validation-batch",
        "verify-artifacts",
        "--provenance-log",
        str(log_path),
        "--run-id",
        "run-freshness",
        "--json",
    )

    assert result.returncode == 1
    payload = json.loads(result.stdout)
    assert payload["verdict"] == "missing"
    assert "missing_output" in {diagnostic["code"] for diagnostic in payload["diagnostics"]}


def test_verify_artifacts_fails_after_input_mutation(tmp_path: Path) -> None:
    log_path, input_path, _output_path, _entry = _provenance_fixture(tmp_path)
    input_path.write_text('{"state":"changed"}\n', encoding="utf-8")

    result = _run_cli(
        "validation-batch",
        "verify-artifacts",
        "--provenance-log",
        str(log_path),
        "--run-id",
        "run-freshness",
        "--json",
    )

    assert result.returncode == 1
    payload = json.loads(result.stdout)
    assert payload["verdict"] == "stale"
    assert "input_hash_changed" in {diagnostic["code"] for diagnostic in payload["diagnostics"]}


def test_verify_artifacts_invalid_selection_is_redacted(tmp_path: Path) -> None:
    log_path, _input_path, _output_path, _entry = _provenance_fixture(tmp_path)

    result = _run_cli(
        "validation-batch",
        "verify-artifacts",
        "--provenance-log",
        str(log_path),
        "--run-id",
        "missing-run",
        "--json",
    )

    assert result.returncode == 1
    payload = json.loads(result.stdout)
    assert payload["verdict"] == "invalid_provenance"
    assert payload["production_import_attempted"] is False
    assert payload["ladybugdb_written"] is False
    assert "RAW_FIXTURE_SENTINEL" not in result.stdout
    assert "secret-token" not in result.stdout
