from __future__ import annotations

from dataclasses import FrozenInstanceError

import pytest

from research_graph.domain.ports import (
    KnowledgeGraphProjectionPort,
    ProjectionDiagnostic,
    ProjectionEdgeRef,
    ProjectionNodeRef,
    ProjectionRequest,
    ProjectionResult,
)
from research_graph.workflows.universal_kb.contracts import CandidatePacket, SafetyFlags


class FakeProjectionAdapter:
    def project(self, request: ProjectionRequest) -> ProjectionResult:
        return ProjectionResult(
            schema_version=request.schema_version,
            backend="fake_projection",
            node_refs=(ProjectionNodeRef(ref="node:paper:1", node_type="paper"),),
            edge_refs=(
                ProjectionEdgeRef(
                    ref="edge:paper:1->claim:1",
                    edge_type="supports",
                    source_ref="node:paper:1",
                    target_ref="node:claim:1",
                ),
            ),
            evidence_refs=request.candidate_packet.evidence_refs,
            provenance_refs=request.candidate_packet.provenance_refs,
            diagnostics=(ProjectionDiagnostic(code="projection_shape_checked", phase="contract"),),
        )


def _candidate_packet() -> CandidatePacket:
    return CandidatePacket(
        candidate_id="candidate-1",
        evidence_refs=("artifact:evidence-1",),
        candidate_type="graph_candidate",
        schema_version="universal-kb-candidate.v1",
        graph_node_refs=("node:paper:1",),
        graph_edge_refs=("edge:paper:1->claim:1",),
        provenance_refs=("source:arxiv:2605.18747",),
    )


def test_fake_projection_adapter_satisfies_port_and_preserves_no_write() -> None:
    adapter = FakeProjectionAdapter()
    request = ProjectionRequest(candidate_packet=_candidate_packet())

    assert isinstance(adapter, KnowledgeGraphProjectionPort)
    result = adapter.project(request)

    assert result.schema_version == "knowledge-graph-projection.v1"
    assert result.backend == "fake_projection"
    assert result.node_refs[0].ref == "node:paper:1"
    assert result.edge_refs[0].source_ref == "node:paper:1"
    assert result.evidence_refs == ("artifact:evidence-1",)
    assert result.provenance_refs == ("source:arxiv:2605.18747",)
    assert result.diagnostics[0].code == "projection_shape_checked"
    assert result.safety_flags == SafetyFlags()
    result.assert_no_write()


def test_projection_result_rejects_write_authority() -> None:
    with pytest.raises(ValueError, match="M034 forbids"):
        ProjectionResult(
            schema_version="knowledge-graph-projection.v1",
            backend="fake_projection",
            safety_flags=SafetyFlags(graphdb_written=True),
        )


def test_projection_contract_records_are_frozen_and_metadata_only() -> None:
    node = ProjectionNodeRef(ref="node:paper:1", node_type="paper")
    edge = ProjectionEdgeRef(
        ref="edge:paper:1->claim:1",
        edge_type="supports",
        source_ref="node:paper:1",
        target_ref="node:claim:1",
    )
    diagnostic = ProjectionDiagnostic(code="shape_checked", phase="projection")
    result = ProjectionResult(
        schema_version="knowledge-graph-projection.v1",
        backend="fake_projection",
        node_refs=(node,),
        edge_refs=(edge,),
        diagnostics=(diagnostic,),
    )

    dumped = result.to_dict()
    assert dumped["node_refs"][0]["ref"] == "node:paper:1"
    assert dumped["edge_refs"][0]["target_ref"] == "node:claim:1"
    assert dumped["diagnostics"][0]["phase"] == "projection"
    assert dumped["safety_flags"]["import_eligible"] is False
    assert "raw_text" not in str(dumped)
    assert "embedding" not in str(dumped)

    with pytest.raises(FrozenInstanceError):
        node.ref = "node:paper:2"  # type: ignore[misc]  # ty:ignore[invalid-assignment]


def test_projection_contract_rejects_raw_or_secret_metadata() -> None:
    with pytest.raises(ValueError, match="node ref must be metadata-only"):
        ProjectionNodeRef(ref="raw text payload", node_type="paper")

    with pytest.raises(ValueError, match="diagnostic code must be metadata-only"):
        ProjectionDiagnostic(code="api_key", phase="projection")

    with pytest.raises(ValueError, match="backend must be metadata-only"):
        ProjectionResult(
            schema_version="knowledge-graph-projection.v1",
            backend="sk-live-abc1234567890",
        )


def test_projection_request_requires_candidate_packet_no_write() -> None:
    request = ProjectionRequest(candidate_packet=_candidate_packet())

    assert request.schema_version == "knowledge-graph-projection.v1"
    request.candidate_packet.assert_no_write()
    with pytest.raises(FrozenInstanceError):
        request.schema_version = "other"  # type: ignore[misc]  # ty:ignore[invalid-assignment]
