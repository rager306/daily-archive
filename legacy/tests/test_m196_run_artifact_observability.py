from __future__ import annotations

import json
from pathlib import Path

from research_graph.workflows.universal_kb.rehearsal import run_universal_kb_no_write_rehearsal

EXPECTED_ARTIFACTS = {
    "candidate.json",
    "review_packet.json",
    "review_trace.json",
    "queue_inspect.json",
    "readiness_handoff.json",
    "schema_gate_result.json",
    "projection_result.json",
    "summary.json",
}
FORBIDDEN_TERMS = (
    "api_key",
    "secret_value",
    "raw_prompt_payload",
    "paper_text_payload",
    "chunk_text_payload",
    "embedding_payload",
    "vector_payload",
)


def _read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def test_no_write_rehearsal_run_artifacts_are_operator_readable(tmp_path: Path) -> None:
    result = run_universal_kb_no_write_rehearsal(tmp_path)
    artifact_names = {path.name for path in result.artifact_paths}

    assert artifact_names == EXPECTED_ARTIFACTS

    queue = _read_json(tmp_path / "queue_inspect.json")
    handoff = _read_json(tmp_path / "readiness_handoff.json")
    schema_gate = _read_json(tmp_path / "schema_gate_result.json")
    projection = _read_json(tmp_path / "projection_result.json")
    summary = _read_json(tmp_path / "summary.json")

    assert queue["job"]["status"] == "ready"
    assert queue["job"]["stage"] == "review_assistance"
    assert "attempt_count" in queue["job"]
    assert queue["events"]

    assert handoff["dry_run_only"] is True
    assert handoff["graph_write_allowed"] is False
    assert handoff["promotion_allowed"] is False
    assert handoff["production_import_attempted"] is False
    assert handoff["safety_flags"]["import_eligible"] is False

    assert schema_gate["accepted"] is True
    assert schema_gate["migration_required"] is False
    assert schema_gate["diagnostics"] == ["schema_versions_current"]
    assert schema_gate["safety_flags"]["import_eligible"] is False

    assert projection["backend"] == "networkx"
    assert projection["diagnostics"]
    assert projection["safety_flags"]["graphdb_written"] is False
    assert projection["safety_flags"]["import_eligible"] is False

    assert summary["candidate_id"] == "sidecar-candidate-1"
    assert summary["queue_job_id"] == "sidecar-candidate-1"
    assert summary["queue_status"] == "ready"
    assert summary["artifact_count"] == 7
    assert summary["schema_gate_accepted"] is True
    assert summary["projection_backend"] == "networkx"
    assert summary["projection_import_eligible"] is False
    assert set(summary["artifact_paths"]) == EXPECTED_ARTIFACTS - {"summary.json"}


def test_no_write_rehearsal_run_artifacts_do_not_leak_payloads(tmp_path: Path) -> None:
    result = run_universal_kb_no_write_rehearsal(tmp_path)

    for path in result.artifact_paths:
        payload = path.read_text(encoding="utf-8").lower()
        for term in FORBIDDEN_TERMS:
            assert term not in payload, f"{path.name} contains forbidden term {term!r}"
