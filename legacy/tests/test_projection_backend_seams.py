from __future__ import annotations

from research_graph.domain.ports import KnowledgeGraphProjectionPort, ProjectionRequest
from research_graph.infrastructure.graph.projection_backends import (
    DisabledBackendProjectionAdapter,
    DisabledFalkorProjectionAdapter,
    DisabledLadybugProjectionAdapter,
)
from research_graph.workflows.universal_kb.contracts import CandidatePacket, SafetyFlags


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


def test_disabled_ladybug_projection_adapter_is_port_compliant_and_no_write() -> None:
    adapter = DisabledLadybugProjectionAdapter()

    assert isinstance(adapter, KnowledgeGraphProjectionPort)
    result = adapter.project(ProjectionRequest(candidate_packet=_candidate_packet()))

    assert result.backend == "ladybugdb"
    assert result.node_refs == ()
    assert result.edge_refs == ()
    assert result.diagnostics[0].code == "backend_projection_disabled"
    assert result.diagnostics[0].phase == "ladybugdb_projection"
    assert result.safety_flags == SafetyFlags()
    result.assert_no_write()


def test_disabled_falkor_projection_adapter_is_port_compliant_and_no_write() -> None:
    adapter = DisabledFalkorProjectionAdapter()

    assert isinstance(adapter, KnowledgeGraphProjectionPort)
    result = adapter.project(ProjectionRequest(candidate_packet=_candidate_packet()))

    assert result.backend == "falkordb"
    assert result.node_refs == ()
    assert result.edge_refs == ()
    assert result.diagnostics[0].code == "backend_projection_disabled"
    assert result.diagnostics[0].phase == "falkordb_projection"
    assert result.safety_flags.import_eligible is False


def test_backend_projection_dry_run_echoes_metadata_refs_only() -> None:
    adapter = DisabledBackendProjectionAdapter(backend="ladybugdb", dry_run=True)

    result = adapter.project(ProjectionRequest(candidate_packet=_candidate_packet()))

    assert result.backend == "ladybugdb"
    assert [node.ref for node in result.node_refs] == ["node:paper:1"]
    assert [edge.ref for edge in result.edge_refs] == ["edge:paper:1->claim:1"]
    assert result.evidence_refs == ("artifact:evidence-1",)
    assert result.provenance_refs == ("source:arxiv:2605.18747",)
    assert result.diagnostics[0].code == "backend_projection_dry_run"
    assert "raw_text" not in str(result.to_dict())
    assert "embedding" not in str(result.to_dict())
    result.assert_no_write()


def test_backend_projection_seams_reject_unsafe_backend_names() -> None:
    result = DisabledBackendProjectionAdapter(backend="api_key").project(
        ProjectionRequest(candidate_packet=_candidate_packet())
    )

    assert result.backend == "disabled_backend"
    assert result.diagnostics[0].code == "backend_projection_configuration_invalid"
    assert result.safety_flags.import_eligible is False
