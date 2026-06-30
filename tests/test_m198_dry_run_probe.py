from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DRY_RUN = ROOT / "scripts/run_m197_reactive_dry_run.py"
PROBE = ROOT / "scripts/run_m198_dry_run_probe.py"
CONTRACT = ROOT / "data/architecture-assessment/m198-readiness-evidence-contract.json"


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _run_dry_run(events_path: Path) -> None:
    subprocess.run(
        [sys.executable, str(DRY_RUN), "--events", str(events_path)],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=True,
    )


def _run_probe(events_path: Path, evidence_path: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(PROBE),
            "--events",
            str(events_path),
            "--evidence",
            str(evidence_path),
            "--correlation-id",
            "corr-probe",
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )


def test_dry_run_probe_writes_contract_shaped_evidence(tmp_path: Path) -> None:
    events_path = tmp_path / "events.jsonl"
    evidence_path = tmp_path / "evidence.json"
    _run_dry_run(events_path)

    completed = _run_probe(events_path, evidence_path)

    assert completed.returncode == 0, completed.stderr
    assert "event_count=4" in completed.stdout
    evidence = _load(evidence_path)
    contract = _load(CONTRACT)
    for field in contract["required_fields"]:
        assert field in evidence
    assert evidence["schema_version"] == contract["schema_version"]
    assert evidence["source_kind"] == "reactive_dry_run"
    assert evidence["correlation_id"] == "corr-probe"
    assert evidence["status"] == "pass"
    assert evidence["drift_class"] == "not_applicable"
    assert evidence["graph_writes_allowed"] is False
    assert evidence["schema_migration_allowed"] is False
    assert evidence["import_eligible"] is False
    assert evidence["event_count"] == 4
    assert evidence["diagnostics"]["completed_stage_count"] == 2
    assert evidence["diagnostics"]["queue_artifact_present"] is False
    assert evidence["diagnostics"]["standalone_queue_events_present"] is False
    assert evidence["source_artifact_refs"] == [
        "projection_safety_result.json",
        "schema_gate_result.json",
    ]


def test_dry_run_probe_rejects_missing_events_file(tmp_path: Path) -> None:
    completed = _run_probe(tmp_path / "missing.jsonl", tmp_path / "evidence.json")

    assert completed.returncode != 0
    assert "events file not found" in completed.stderr
    assert not (tmp_path / "evidence.json").exists()


def test_dry_run_probe_rejects_bad_write_flags(tmp_path: Path) -> None:
    events_path = tmp_path / "events.jsonl"
    evidence_path = tmp_path / "evidence.json"
    _run_dry_run(events_path)
    events = [json.loads(line) for line in events_path.read_text(encoding="utf-8").splitlines()]
    events[0]["graph_writes_allowed"] = True
    events_path.write_text("\n".join(json.dumps(event) for event in events) + "\n", encoding="utf-8")

    completed = _run_probe(events_path, evidence_path)

    assert completed.returncode != 0
    assert "graph_writes_allowed must be false" in completed.stderr
    assert not evidence_path.exists()


def test_dry_run_probe_rejects_forbidden_payload_terms(tmp_path: Path) -> None:
    events_path = tmp_path / "events.jsonl"
    evidence_path = tmp_path / "evidence.json"
    _run_dry_run(events_path)
    events = [json.loads(line) for line in events_path.read_text(encoding="utf-8").splitlines()]
    events[0]["diagnostics"]["leak"] = "api_key"
    events_path.write_text("\n".join(json.dumps(event) for event in events) + "\n", encoding="utf-8")

    completed = _run_probe(events_path, evidence_path)

    assert completed.returncode != 0
    assert "forbidden payload term found: api_key" in completed.stderr
    assert not evidence_path.exists()
