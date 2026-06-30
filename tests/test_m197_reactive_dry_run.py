from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m197-reactive-event-contract.json"
SCRIPT = ROOT / "scripts/run_m197_reactive_dry_run.py"


def _load_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def _contract() -> dict:
    return json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))


def _assert_contract_event(event: dict) -> None:
    contract = _contract()
    for field in contract["required_fields"]:
        assert field in event
    assert event["schema_version"] == contract["schema_version"]
    assert event["graph_writes_allowed"] is False
    assert event["schema_migration_allowed"] is False
    assert event["import_eligible"] is False


def test_reactive_dry_run_writes_contract_jsonl_to_explicit_path(tmp_path: Path) -> None:
    events_path = tmp_path / "events.jsonl"

    completed = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--events",
            str(events_path),
            "--job-id",
            "job-cli",
            "--correlation-id",
            "corr-cli",
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=True,
    )

    assert "m197_reactive_events=4" in completed.stdout
    assert events_path.exists()
    events = _load_jsonl(events_path)
    assert [event["event_type"] for event in events] == [
        "stage.started",
        "stage.completed",
        "stage.started",
        "stage.completed",
    ]
    assert [event["stage_id"] for event in events] == [
        "dry_run.schema_gate",
        "dry_run.schema_gate",
        "dry_run.projection_safety",
        "dry_run.projection_safety",
    ]
    for event in events:
        _assert_contract_event(event)
        assert event["job_id"] == "job-cli"
        assert event["correlation_id"] == "corr-cli"


def test_reactive_dry_run_events_are_lineage_metadata_only(tmp_path: Path) -> None:
    events_path = tmp_path / "events.jsonl"
    subprocess.run(
        [sys.executable, str(SCRIPT), "--events", str(events_path)],
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    )

    events = _load_jsonl(events_path)
    completed = [event for event in events if event["event_type"] == "stage.completed"]
    assert completed[0]["parent_artifact_refs"] == ["operator_dry_run_request.json"]
    assert completed[0]["child_artifact_refs"] == ["schema_gate_result.json"]
    assert completed[0]["checksum_sha256"] == "1" * 64
    assert completed[1]["parent_artifact_refs"] == ["schema_gate_result.json"]
    assert completed[1]["child_artifact_refs"] == ["projection_safety_result.json"]
    assert completed[1]["checksum_sha256"] == "2" * 64

    event_text = json.dumps(events).lower()
    for term in _contract()["forbidden_payload_terms"]:
        assert term.lower() not in event_text


def test_reactive_dry_run_rejects_invalid_concurrency(tmp_path: Path) -> None:
    events_path = tmp_path / "events.jsonl"
    completed = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--events",
            str(events_path),
            "--max-concurrency",
            "0",
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )

    assert completed.returncode != 0
    assert not events_path.exists()
    assert "max_concurrency must be >= 1" in completed.stderr
