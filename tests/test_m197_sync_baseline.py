from __future__ import annotations

import json
from pathlib import Path

from research_graph.workflows.universal_kb.rehearsal import run_universal_kb_no_write_rehearsal

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m197-reactive-event-contract.json"
EXPECTED_ARTIFACTS = {
    "candidate.json",
    "projection_result.json",
    "queue_inspect.json",
    "readiness_handoff.json",
    "review_packet.json",
    "review_trace.json",
    "schema_gate_result.json",
    "summary.json",
}


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def test_sync_no_write_rehearsal_baseline_outputs_expected_artifacts(tmp_path: Path) -> None:
    result = run_universal_kb_no_write_rehearsal(tmp_path)

    artifact_names = {path.name for path in result.artifact_paths}
    assert artifact_names == EXPECTED_ARTIFACTS
    assert (tmp_path / "queue.sqlite").exists()
    assert not (tmp_path / "queue_events.json").exists()


def test_sync_no_write_rehearsal_baseline_keeps_safety_flags_false(tmp_path: Path) -> None:
    run_universal_kb_no_write_rehearsal(tmp_path)

    queue = _load(tmp_path / "queue_inspect.json")
    schema = _load(tmp_path / "schema_gate_result.json")
    projection = _load(tmp_path / "projection_result.json")
    summary = _load(tmp_path / "summary.json")

    assert queue["job"]["status"] == "ready"
    assert schema["accepted"] is True
    assert schema["migration_required"] is False
    assert schema["diagnostics"] == ["schema_versions_current"]
    assert projection["backend"] == "networkx"
    assert projection["safety_flags"]["graphdb_written"] is False
    assert projection["safety_flags"]["ladybugdb_written"] is False
    assert projection["safety_flags"]["production_import_attempted"] is False
    assert projection["safety_flags"]["graph_import_allowed"] is False
    assert projection["safety_flags"]["import_eligible"] is False
    assert summary["schema_gate_accepted"] is True
    assert summary["projection_import_eligible"] is False


def test_sync_baseline_can_populate_required_future_event_fields(tmp_path: Path) -> None:
    result = run_universal_kb_no_write_rehearsal(tmp_path)
    contract = _load(CONTRACT_PATH)
    queue = _load(tmp_path / "queue_inspect.json")
    projection = _load(tmp_path / "projection_result.json")

    event_candidate = {
        "schema_version": contract["schema_version"],
        "event_type": "stage.completed",
        "job_id": queue["job"]["job_id"],
        "stage_id": "sync.no_write_rehearsal",
        "correlation_id": "sync-baseline",
        "phase": "projection",
        "status": "completed",
        "attempt": queue["job"].get("attempt_count", 0),
        "timestamp": queue["job"]["updated_at"],
        "graph_writes_allowed": False,
        "schema_migration_allowed": False,
        "import_eligible": projection["safety_flags"]["import_eligible"],
        "artifact_refs": sorted(path.name for path in result.artifact_paths),
        "diagnostics": projection["diagnostics"],
    }

    for field in contract["required_fields"]:
        assert field in event_candidate
    assert event_candidate["import_eligible"] is False
    assert event_candidate["artifact_refs"] == sorted(EXPECTED_ARTIFACTS)


def test_sync_baseline_json_artifacts_do_not_contain_payload_shaped_forbidden_terms(tmp_path: Path) -> None:
    result = run_universal_kb_no_write_rehearsal(tmp_path)
    forbidden = set(_load(CONTRACT_PATH)["forbidden_payload_terms"])

    for artifact_path in result.artifact_paths:
        text = artifact_path.read_text(encoding="utf-8")
        lowered = text.lower()
        for term in forbidden:
            assert term.lower() not in lowered, artifact_path.name
