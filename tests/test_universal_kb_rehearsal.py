from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.infrastructure.llm.minimax_structured import DEFAULT_MINIMAX_MODEL
from research_graph.workflows.universal_kb.rehearsal import (
    RehearsalResult,
    run_universal_kb_no_write_rehearsal,
)

_FORBIDDEN_PERSISTED_TERMS = (
    "api_key",
    "secret_value",
    "embedding_payload",
    "vector_payload",
    "chunk_text_payload",
    "paper_text_payload",
    "claim_text_payload",
)


def _read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def test_no_write_rehearsal_runs_end_to_end_and_writes_metadata_artifacts(tmp_path: Path) -> None:
    result = run_universal_kb_no_write_rehearsal(tmp_path)

    assert isinstance(result, RehearsalResult)
    assert result.candidate_id == "sidecar-candidate-1"
    assert result.queue_job_id == "sidecar-candidate-1"
    assert result.model == DEFAULT_MINIMAX_MODEL == "MiniMax-M3-512k"
    assert result.artifact_paths == (
        tmp_path / "candidate.json",
        tmp_path / "review_packet.json",
        tmp_path / "review_trace.json",
        tmp_path / "queue_inspect.json",
        tmp_path / "readiness_handoff.json",
        tmp_path / "schema_gate_result.json",
        tmp_path / "projection_result.json",
        tmp_path / "summary.json",
    )

    for path in result.artifact_paths:
        assert path.exists(), path

    handoff = _read_json(tmp_path / "readiness_handoff.json")
    assert handoff["dry_run_only"] is True
    assert handoff["graph_write_allowed"] is False
    assert handoff["promotion_allowed"] is False
    assert handoff["production_import_attempted"] is False
    assert handoff["safety_flags"]["graphdb_written"] is False
    assert handoff["safety_flags"]["ladybugdb_written"] is False
    assert handoff["safety_flags"]["graph_import_allowed"] is False
    assert handoff["safety_flags"]["import_eligible"] is False
    assert handoff["readiness_state"] in {"pending", "diagnostics_only"}

    queue_inspect = _read_json(tmp_path / "queue_inspect.json")
    assert queue_inspect["job"]["status"] == "ready"
    assert queue_inspect["job"]["tool_version"] == "MiniMax-M3-512k"
    assert queue_inspect["events"][-1]["event_type"] == "unblock"

    schema_gate = _read_json(tmp_path / "schema_gate_result.json")
    assert schema_gate["accepted"] is True
    assert schema_gate["migration_required"] is False
    assert schema_gate["diagnostics"] == ["schema_versions_current"]
    assert schema_gate["candidate_schema_version"] == "universal-kb-candidate.v1"
    assert schema_gate["projection_schema_version"] == "knowledge-graph-projection.v1"
    assert schema_gate["safety_flags"]["graphdb_written"] is False
    assert schema_gate["safety_flags"]["ladybugdb_written"] is False
    assert schema_gate["safety_flags"]["import_eligible"] is False

    projection = _read_json(tmp_path / "projection_result.json")
    assert projection["backend"] == "networkx"
    assert projection["safety_flags"]["graphdb_written"] is False
    assert projection["safety_flags"]["ladybugdb_written"] is False
    assert projection["safety_flags"]["import_eligible"] is False
    assert projection["diagnostics"][0]["code"] in {
        "networkx_projection_completed",
        "networkx_projection_failed",
    }

    summary = _read_json(tmp_path / "summary.json")
    assert summary["graph_write_allowed"] is False
    assert summary["promotion_allowed"] is False
    assert summary["production_import_attempted"] is False
    assert summary["artifact_count"] == 7
    assert summary["schema_gate_accepted"] is True
    assert summary["schema_gate_migration_required"] is False
    assert summary["schema_gate_diagnostics"] == ["schema_versions_current"]
    assert summary["projection_backend"] == "networkx"
    assert summary["projection_import_eligible"] is False
    assert summary["projection_diagnostics"][0] in {
        "networkx_projection_completed",
        "networkx_projection_failed",
    }


def test_no_write_rehearsal_artifacts_are_metadata_only(tmp_path: Path) -> None:
    result = run_universal_kb_no_write_rehearsal(tmp_path)

    for path in result.artifact_paths:
        payload = path.read_text(encoding="utf-8").lower()
        for term in _FORBIDDEN_PERSISTED_TERMS:
            assert term not in payload, f"{path} persisted forbidden term {term!r}"

    trace = _read_json(tmp_path / "review_trace.json")
    assert trace["helper_evidence_only"] is True
    assert trace["minimax_source_of_truth"] is False
    assert trace["raw_prompt_persisted"] is False
    assert trace["credential_value_logged"] is False


def test_no_write_rehearsal_refuses_existing_artifact_directory(tmp_path: Path) -> None:
    (tmp_path / "summary.json").write_text("{}", encoding="utf-8")

    with pytest.raises(FileExistsError, match="summary.json"):
        run_universal_kb_no_write_rehearsal(tmp_path)
