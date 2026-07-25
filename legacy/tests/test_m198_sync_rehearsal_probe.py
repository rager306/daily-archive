from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from research_graph.workflows.universal_kb.rehearsal import run_universal_kb_no_write_rehearsal

ROOT = Path(__file__).resolve().parents[1]
PROBE = ROOT / "scripts/run_m198_sync_rehearsal_probe.py"
CONTRACT = ROOT / "data/architecture-assessment/m198-readiness-evidence-contract.json"


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _run_probe(artifact_dir: Path, evidence_path: Path, *, skip_run: bool = False) -> subprocess.CompletedProcess[str]:
    command = [
        sys.executable,
        str(PROBE),
        "--artifact-dir",
        str(artifact_dir),
        "--evidence",
        str(evidence_path),
        "--correlation-id",
        "corr-sync",
    ]
    if skip_run:
        command.append("--skip-run")
    return subprocess.run(command, cwd=ROOT, text=True, capture_output=True)


def _prepare_rehearsal(artifact_dir: Path) -> None:
    run_universal_kb_no_write_rehearsal(artifact_dir)


def test_sync_rehearsal_probe_writes_contract_shaped_evidence(tmp_path: Path) -> None:
    artifact_dir = tmp_path / "rehearsal"
    evidence_path = tmp_path / "evidence.json"

    completed = _run_probe(artifact_dir, evidence_path)

    assert completed.returncode == 0, completed.stderr
    assert "queue_artifact_present=True" in completed.stdout
    assert "standalone_queue_events_present=False" in completed.stdout
    evidence = _load(evidence_path)
    contract = _load(CONTRACT)
    for field in contract["required_fields"]:
        assert field in evidence
    assert evidence["schema_version"] == contract["schema_version"]
    assert evidence["source_kind"] == "sync_no_write_rehearsal"
    assert evidence["correlation_id"] == "corr-sync"
    assert evidence["status"] == "pass"
    assert evidence["drift_class"] == "not_applicable"
    assert evidence["graph_writes_allowed"] is False
    assert evidence["schema_migration_allowed"] is False
    assert evidence["import_eligible"] is False
    assert evidence["queue_artifact_present"] is True
    assert evidence["standalone_queue_events_present"] is False
    assert evidence["diagnostics"]["queue_sqlite_present"] is True
    assert evidence["diagnostics"]["standalone_queue_events_present"] is False
    assert evidence["diagnostics"]["schema_gate_migration_required"] is False
    assert evidence["diagnostics"]["projection_import_eligible"] is False
    assert any(ref.endswith("queue.sqlite") for ref in evidence["evidence_refs"])
    assert any(ref.endswith("queue_inspect.json") for ref in evidence["evidence_refs"])
    assert not (artifact_dir / "queue_events.json").exists()


def test_sync_rehearsal_probe_rejects_missing_summary(tmp_path: Path) -> None:
    artifact_dir = tmp_path / "rehearsal"
    evidence_path = tmp_path / "evidence.json"
    _prepare_rehearsal(artifact_dir)
    (artifact_dir / "summary.json").unlink()

    completed = _run_probe(artifact_dir, evidence_path, skip_run=True)

    assert completed.returncode != 0
    assert "required rehearsal artifact missing: summary.json" in completed.stderr
    assert not evidence_path.exists()


def test_sync_rehearsal_probe_rejects_bad_write_flag(tmp_path: Path) -> None:
    artifact_dir = tmp_path / "rehearsal"
    evidence_path = tmp_path / "evidence.json"
    _prepare_rehearsal(artifact_dir)
    summary_path = artifact_dir / "summary.json"
    summary = _load(summary_path)
    summary["graph_write_allowed"] = True
    summary_path.write_text(json.dumps(summary), encoding="utf-8")

    completed = _run_probe(artifact_dir, evidence_path, skip_run=True)

    assert completed.returncode != 0
    assert "graph_write_allowed must be false" in completed.stderr
    assert not evidence_path.exists()


def test_sync_rehearsal_probe_rejects_promotion_or_import_leakage(tmp_path: Path) -> None:
    artifact_dir = tmp_path / "rehearsal"
    evidence_path = tmp_path / "evidence.json"
    _prepare_rehearsal(artifact_dir)
    summary_path = artifact_dir / "summary.json"
    summary = _load(summary_path)
    summary["promotion_allowed"] = True
    summary_path.write_text(json.dumps(summary), encoding="utf-8")

    completed = _run_probe(artifact_dir, evidence_path, skip_run=True)

    assert completed.returncode != 0
    assert "promotion_allowed must be false" in completed.stderr
    assert not evidence_path.exists()


def test_sync_rehearsal_probe_rejects_forbidden_payload_terms(tmp_path: Path) -> None:
    artifact_dir = tmp_path / "rehearsal"
    evidence_path = tmp_path / "evidence.json"
    _prepare_rehearsal(artifact_dir)
    summary_path = artifact_dir / "summary.json"
    summary = _load(summary_path)
    summary["diagnostics"] = {"leak": "api_key"}
    summary_path.write_text(json.dumps(summary), encoding="utf-8")

    completed = _run_probe(artifact_dir, evidence_path, skip_run=True)

    assert completed.returncode != 0
    assert "forbidden payload term found: api_key" in completed.stderr
    assert not evidence_path.exists()
