from __future__ import annotations

import json
from datetime import UTC, datetime, timedelta
from pathlib import Path

import pytest

from arxiv_archive.validation_batch_provenance import (
    append_validation_cli_provenance,
    build_artifact_freshness_report,
    build_validation_cli_provenance_entry,
    fingerprint_file,
    read_validation_cli_provenance_log,
    redact_cli_args,
    select_provenance_entry,
    write_artifact_freshness_report,
)


def _entry(tmp_path: Path) -> tuple[dict, Path, Path]:
    input_path = tmp_path / "input-state.json"
    output_path = tmp_path / "validation-scan-summary.json"
    input_path.write_text('{"state":"ready","sentinel":"RAW_SECRET_TEXT"}\n', encoding="utf-8")
    output_path.write_text('{"summary":"ok"}\n', encoding="utf-8")
    started = datetime(2026, 5, 20, 4, 0, 0, tzinfo=UTC)
    completed = started + timedelta(seconds=2)
    entry = build_validation_cli_provenance_entry(
        command="validation-batch scan",
        argv=["validation-batch", "scan", "--api-key", "secret-value", "--state-path", str(input_path)],
        batch_id="fixture-batch",
        input_paths=[input_path],
        output_paths=[output_path],
        status="scanned",
        started_at=started,
        completed_at=completed,
        exit_code=0,
        cwd=tmp_path,
        run_id="run-001",
        real_scan_performed=True,
    )
    return entry, input_path, output_path


def test_fingerprint_file_hashes_bytes_without_content(tmp_path: Path) -> None:
    path = tmp_path / "paper.md"
    path.write_text("RAW_SENTINEL_TEXT", encoding="utf-8")

    payload = fingerprint_file(path)

    dumped = json.dumps(payload)
    assert payload["sha256"].startswith("sha256:")
    assert payload["size_bytes"] == len("RAW_SENTINEL_TEXT")
    assert "RAW_SENTINEL_TEXT" not in dumped


def test_redact_cli_args_removes_secret_like_values() -> None:
    redacted = redact_cli_args([
        "cmd",
        "--api-key",
        "abc",
        "--token=def",
        "--state-path",
        "state.json",
    ])

    dumped = " ".join(redacted)
    assert "abc" not in dumped
    assert "def" not in dumped
    assert "--api-key" in redacted
    assert "--token=<redacted>" in redacted
    assert "state.json" in redacted


def test_build_provenance_entry_records_hashes_and_safety_flags(tmp_path: Path) -> None:
    entry, _input_path, _output_path = _entry(tmp_path)

    dumped = json.dumps(entry)
    assert entry["schema_version"] == "m009-validation-cli-provenance.v1"
    assert entry["batch_id"] == "fixture-batch"
    assert entry["command"] == "validation-batch scan"
    assert entry["duration_ms"] == 2000
    assert entry["inputs"][0]["sha256"].startswith("sha256:")
    assert entry["outputs"][0]["sha256"].startswith("sha256:")
    assert entry["raw_text_included"] is False
    assert entry["chunk_text_included"] is False
    assert entry["embeddings_included"] is False
    assert entry["vectors_included"] is False
    assert entry["secrets_included"] is False
    assert "RAW_SECRET_TEXT" not in dumped
    assert "secret-value" not in dumped


def test_append_and_read_provenance_jsonl_round_trip(tmp_path: Path) -> None:
    entry, _input_path, _output_path = _entry(tmp_path)
    second = {**entry, "run_id": "run-002", "completed_at": "2026-05-20T04:00:03Z"}
    log_path = tmp_path / "cli-run-log.jsonl"

    append_validation_cli_provenance(log_path, entry)
    append_validation_cli_provenance(log_path, second)
    entries = read_validation_cli_provenance_log(log_path)

    assert [item["run_id"] for item in entries] == ["run-001", "run-002"]
    assert select_provenance_entry(entries, run_id="run-001")["run_id"] == "run-001"
    assert select_provenance_entry(entries, batch_id="fixture-batch", command="validation-batch scan")["run_id"] == "run-002"


def test_freshness_report_passes_for_unchanged_files(tmp_path: Path) -> None:
    entry, _input_path, _output_path = _entry(tmp_path)

    report = build_artifact_freshness_report(entry)

    assert report["verdict"] == "fresh"
    assert report["missing_count"] == 0
    assert report["mismatch_count"] == 0
    assert report["diagnostics"] == []


def test_freshness_report_fails_when_output_changes(tmp_path: Path) -> None:
    entry, _input_path, output_path = _entry(tmp_path)
    output_path.write_text('{"summary":"mutated"}\n', encoding="utf-8")

    report = build_artifact_freshness_report(entry)

    assert report["verdict"] == "stale"
    assert report["mismatch_count"] >= 1
    assert {diagnostic["code"] for diagnostic in report["diagnostics"]} >= {"output_hash_changed"}


def test_freshness_report_fails_when_output_missing(tmp_path: Path) -> None:
    entry, _input_path, output_path = _entry(tmp_path)
    output_path.unlink()

    report = build_artifact_freshness_report(entry)

    assert report["verdict"] == "missing"
    assert report["missing_count"] == 1
    assert {diagnostic["code"] for diagnostic in report["diagnostics"]} == {"missing_output"}


def test_freshness_report_rejects_unsafe_provenance_flags(tmp_path: Path) -> None:
    entry, _input_path, _output_path = _entry(tmp_path)
    entry["chunk_text_included"] = True

    report = build_artifact_freshness_report(entry)

    assert report["verdict"] == "invalid_provenance"
    assert "unsafe_safety_flag" in {diagnostic["code"] for diagnostic in report["diagnostics"]}


def test_freshness_report_checks_expected_artifact_metadata(tmp_path: Path) -> None:
    input_path = tmp_path / "state.json"
    output_path = tmp_path / "summary.json"
    input_path.write_text('{"state":"ready"}\n', encoding="utf-8")
    output_path.write_text('{"milestone_id":"M009-fh0tg0","batch_id":"batch-1"}\n', encoding="utf-8")
    started = datetime(2026, 5, 20, 4, 0, 0, tzinfo=UTC)
    entry = build_validation_cli_provenance_entry(
        command="validation-batch scan",
        argv=["validation-batch", "scan"],
        batch_id="batch-1",
        input_paths=[input_path],
        output_paths=[output_path],
        status="scanned",
        started_at=started,
        completed_at=started + timedelta(seconds=1),
        expected_artifact_metadata={"milestone_id": "M009-fh0tg0", "batch_id": "batch-1"},
    )

    assert build_artifact_freshness_report(entry)["verdict"] == "fresh"
    output_path.write_text('{"milestone_id":"M006-638rza","batch_id":"batch-1"}\n', encoding="utf-8")
    entry["outputs"] = [fingerprint_file(output_path)]
    report = build_artifact_freshness_report(entry)

    assert report["verdict"] == "stale"
    assert "artifact_metadata_mismatch" in {diagnostic["code"] for diagnostic in report["diagnostics"]}


def test_freshness_report_can_be_written_without_raw_content(tmp_path: Path) -> None:
    entry, _input_path, _output_path = _entry(tmp_path)
    report = build_artifact_freshness_report(entry)
    report_path = tmp_path / "freshness.json"

    write_artifact_freshness_report(report, report_path)

    dumped = report_path.read_text(encoding="utf-8")
    assert "m009-artifact-freshness-report.v1" in dumped
    assert "RAW_SECRET_TEXT" not in dumped


def test_read_provenance_log_rejects_invalid_jsonl(tmp_path: Path) -> None:
    path = tmp_path / "bad.jsonl"
    path.write_text("not-json\n", encoding="utf-8")

    with pytest.raises(ValueError, match="invalid provenance JSON"):
        read_validation_cli_provenance_log(path)
