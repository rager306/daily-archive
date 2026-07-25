"""M203 S02: projection port adequacy for Falkor."""

from __future__ import annotations

from research_graph.application.graph.falkor_capability import probe_falkor_capabilities
from research_graph.application.graph.falkor_projection_adequacy import (
    assess_projection_port_adequacy,
)
from research_graph.domain.ports import KnowledgeGraphProjectionPort, ProjectionRequest
from research_graph.infrastructure.graph.projection_backends import (
    DisabledFalkorProjectionAdapter,
)
from research_graph.workflows.universal_kb.contracts import CandidatePacket


def _packet() -> CandidatePacket:
    return CandidatePacket(
        candidate_id="candidate-falkor-1",
        evidence_refs=("artifact:evidence-1",),
        candidate_type="graph_candidate",
        schema_version="universal-kb-candidate.v1",
        graph_node_refs=("node:paper:1",),
        graph_edge_refs=("edge:paper:1->claim:1",),
        provenance_refs=("source:arxiv:2605.18747",),
    )


def test_adequacy_sufficient_for_current_projection_surface() -> None:
    request = ProjectionRequest(candidate_packet=_packet())
    adapter = DisabledFalkorProjectionAdapter(dry_run=True)
    assert isinstance(adapter, KnowledgeGraphProjectionPort)
    result = adapter.project(request)
    report = assess_projection_port_adequacy(
        request,
        capability=probe_falkor_capabilities(),
        sample_result=result,
    )
    assert report.verdict == "sufficient"
    assert report.graphdb_port_required is False
    report.assert_no_write()
    assert any(f.code == "graphdb_port_not_required_for_no_write" for f in report.findings)


def test_adequacy_report_serializes_without_secrets() -> None:
    request = ProjectionRequest(candidate_packet=_packet())
    report = assess_projection_port_adequacy(request)
    payload = str(report.to_dict()).lower()
    assert "api_key" not in payload
    assert "embedding" not in payload
