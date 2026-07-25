from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CLASSIFIER = ROOT / "scripts/run_m198_drift_classifier.py"
CONTRACT = ROOT / "data/architecture-assessment/m198-readiness-evidence-contract.json"


def _load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _base(kind: str) -> dict[str, Any]:
    return {
        "schema_version": "m198.readiness_evidence.v1",
        "evidence_id": f"evidence-{kind}",
        "source_kind": kind,
        "correlation_id": "corr-source",
        "status": "pass",
        "drift_class": "not_applicable",
        "timestamp": "2026-06-30T00:00:00+00:00",
        "graph_writes_allowed": False,
        "schema_migration_allowed": False,
        "import_eligible": False,
        "evidence_refs": [f"artifact://{kind}.json"],
        "diagnostics": {},
        "non_goals": _load(CONTRACT)["blocked_transitions"],
    }


def _fixtures() -> dict[str, dict[str, Any]]:
    reactive = _base("reactive_dry_run")
    reactive["queue_artifact_present"] = False
    reactive["diagnostics"] = {"event_count": 4}

    sync = _base("sync_no_write_rehearsal")
    sync["queue_artifact_present"] = True
    sync["standalone_queue_events_present"] = False
    sync["diagnostics"] = {"schema_gate_migration_required": False}

    smoke = _base("smoke_boundary")
    smoke["queue_status"] = "ready"
    smoke["diagnostics"] = {"queue_status": "ready", "metadata_only": True}

    graph = _base("graph_readiness_validate_only")
    graph["diagnostics"] = {
        "validate_only": True,
        "require_completed_review": True,
        "validator_ok": True,
        "retired_alias_absent": True,
    }
    return {item["source_kind"]: item for item in (reactive, sync, smoke, graph)}


def _write_evidence(tmp_path: Path, fixtures: dict[str, dict[str, Any]]) -> list[Path]:
    paths: list[Path] = []
    for kind, evidence in fixtures.items():
        path = tmp_path / f"{kind}.json"
        path.write_text(json.dumps(evidence), encoding="utf-8")
        paths.append(path)
    return paths


def _run_classifier(paths: list[Path], report_path: Path) -> subprocess.CompletedProcess[str]:
    command = [sys.executable, str(CLASSIFIER), "--report", str(report_path), "--correlation-id", "corr-drift"]
    for path in paths:
        command.extend(["--evidence", str(path)])
    return subprocess.run(command, cwd=ROOT, text=True, capture_output=True)


def test_drift_classifier_writes_expected_governance_ratchet_report(tmp_path: Path) -> None:
    paths = _write_evidence(tmp_path, _fixtures())
    report_path = tmp_path / "drift.json"

    completed = _run_classifier(paths, report_path)

    assert completed.returncode == 0, completed.stderr
    assert "drift_class=expected" in completed.stdout
    report = _load(report_path)
    contract = _load(CONTRACT)
    for field in contract["required_fields"]:
        assert field in report
    assert report["schema_version"] == contract["schema_version"]
    assert report["source_kind"] == "governance_ratchet"
    assert report["correlation_id"] == "corr-drift"
    assert report["status"] == "pass"
    assert report["drift_class"] == "expected"
    assert report["graph_writes_allowed"] is False
    assert report["schema_migration_allowed"] is False
    assert report["import_eligible"] is False
    assert report["diagnostics"]["blockers"] == []
    assert report["diagnostics"]["warnings"] == []
    assert sorted(report["diagnostics"]["observed_source_kinds"]) == sorted(_fixtures())


def test_drift_classifier_blocks_missing_required_source(tmp_path: Path) -> None:
    fixtures = _fixtures()
    fixtures.pop("smoke_boundary")
    paths = _write_evidence(tmp_path, fixtures)
    report_path = tmp_path / "drift.json"

    completed = _run_classifier(paths, report_path)

    assert completed.returncode == 2
    report = _load(report_path)
    assert report["status"] == "fail"
    assert report["drift_class"] == "blocker"
    assert "missing required source kind: smoke_boundary" in report["diagnostics"]["blockers"]


def test_drift_classifier_blocks_bad_import_flag(tmp_path: Path) -> None:
    fixtures = _fixtures()
    fixtures["sync_no_write_rehearsal"]["import_eligible"] = True
    paths = _write_evidence(tmp_path, fixtures)
    report_path = tmp_path / "drift.json"

    completed = _run_classifier(paths, report_path)

    assert completed.returncode == 2
    report = _load(report_path)
    assert report["drift_class"] == "blocker"
    assert "sync_no_write_rehearsal has import_eligible=True" in report["diagnostics"]["blockers"]


def test_drift_classifier_blocks_failed_source_status(tmp_path: Path) -> None:
    fixtures = _fixtures()
    fixtures["graph_readiness_validate_only"]["status"] = "fail"
    paths = _write_evidence(tmp_path, fixtures)
    report_path = tmp_path / "drift.json"

    completed = _run_classifier(paths, report_path)

    assert completed.returncode == 2
    report = _load(report_path)
    assert "graph_readiness_validate_only status is 'fail'" in report["diagnostics"]["blockers"]


def test_drift_classifier_blocks_forbidden_payload_terms(tmp_path: Path) -> None:
    fixtures = _fixtures()
    fixtures["reactive_dry_run"]["diagnostics"]["leak"] = "api_key"
    paths = _write_evidence(tmp_path, fixtures)
    report_path = tmp_path / "drift.json"

    completed = _run_classifier(paths, report_path)

    assert completed.returncode == 2
    report = _load(report_path)
    assert "reactive_dry_run contains forbidden payload term: api_key" in report["diagnostics"]["blockers"]


def test_drift_classifier_warns_on_extra_source_kind(tmp_path: Path) -> None:
    fixtures = _fixtures()
    extra = _base("disabled_backend")
    extra["diagnostics"] = {"disabled": True}
    fixtures["disabled_backend"] = extra
    paths = _write_evidence(tmp_path, fixtures)
    report_path = tmp_path / "drift.json"

    completed = _run_classifier(paths, report_path)

    assert completed.returncode == 0
    report = _load(report_path)
    assert report["status"] == "pass"
    assert report["drift_class"] == "warning"
    assert report["diagnostics"]["warnings"] == ["extra source kinds ignored: disabled_backend"]
