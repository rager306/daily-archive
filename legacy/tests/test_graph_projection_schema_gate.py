from __future__ import annotations

from research_graph.domain.graph_projection_schema import (
    CURRENT_CANDIDATE_SCHEMA_VERSION,
    CURRENT_PROJECTION_SCHEMA_VERSION,
    GraphProjectionSchemaGate,
)
from research_graph.domain.ports import ProjectionRequest
from research_graph.workflows.universal_kb.contracts import CandidatePacket, SafetyFlags


def _candidate_packet(*, schema_version: str = CURRENT_CANDIDATE_SCHEMA_VERSION) -> CandidatePacket:
    return CandidatePacket(
        candidate_id="candidate-1",
        evidence_refs=("artifact:evidence-1",),
        candidate_type="graph_candidate",
        schema_version=schema_version,
        graph_node_refs=("node:paper:1",),
        graph_edge_refs=("edge:paper:1->claim:1",),
        provenance_refs=("source:arxiv:2605.18747",),
    )


def test_schema_gate_accepts_current_candidate_and_projection_versions() -> None:
    request = ProjectionRequest(candidate_packet=_candidate_packet())

    result = GraphProjectionSchemaGate().validate(request)

    assert result.accepted is True
    assert result.migration_required is False
    assert result.diagnostics == ("schema_versions_current",)
    assert result.migration_plan is None
    assert result.safety_flags == SafetyFlags()
    result.assert_no_write()
    dumped = result.to_dict()
    assert dumped["candidate_schema_version"] == CURRENT_CANDIDATE_SCHEMA_VERSION
    assert dumped["projection_schema_version"] == CURRENT_PROJECTION_SCHEMA_VERSION
    assert dumped["safety_flags"]["import_eligible"] is False


def test_schema_gate_rejects_unsupported_candidate_schema_with_placeholder() -> None:
    request = ProjectionRequest(candidate_packet=_candidate_packet(schema_version="universal-kb-candidate.v0"))

    result = GraphProjectionSchemaGate().validate(request)

    assert result.accepted is False
    assert result.migration_required is True
    assert result.diagnostics == ("schema_migration_required",)
    assert result.migration_plan is not None
    assert result.migration_plan.from_schema_version == "universal-kb-candidate.v0"
    assert result.migration_plan.to_schema_version == CURRENT_CANDIDATE_SCHEMA_VERSION
    assert result.migration_plan.status == "placeholder_only"
    assert result.safety_flags.import_eligible is False


def test_schema_gate_rejects_unsupported_projection_schema_with_placeholder() -> None:
    request = ProjectionRequest(
        candidate_packet=_candidate_packet(),
        schema_version="knowledge-graph-projection.v0",
    )

    result = GraphProjectionSchemaGate().validate(request)

    assert result.accepted is False
    assert result.migration_required is True
    assert result.migration_plan is not None
    assert result.migration_plan.from_schema_version == "knowledge-graph-projection.v0"
    assert result.migration_plan.to_schema_version == CURRENT_PROJECTION_SCHEMA_VERSION
    assert result.safety_flags.graphdb_written is False


def test_schema_gate_outputs_are_metadata_only() -> None:
    request = ProjectionRequest(candidate_packet=_candidate_packet(schema_version="universal-kb-candidate.v0"))

    dumped = GraphProjectionSchemaGate().validate(request).to_dict()

    serialized = str(dumped).lower()
    assert "raw_text" not in serialized
    assert "embedding" not in serialized
    assert "api_key" not in serialized
    assert dumped["migration_plan"]["status"] == "placeholder_only"
