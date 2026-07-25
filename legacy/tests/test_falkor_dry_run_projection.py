"""M203 S03: Falkor dry-run translation + operation plan."""

from __future__ import annotations

from research_graph.application.graph.falkor_dry_run import project_with_falkor_plan
from research_graph.application.graph.falkor_operation_plan import build_falkor_operation_plan
from research_graph.domain.ports import ProjectionRequest
from research_graph.infrastructure.graph.projection_backends import (
    DisabledFalkorProjectionAdapter,
)
from research_graph.workflows.universal_kb.contracts import CandidatePacket


def _packet(cid: str = "candidate-falkor-1") -> CandidatePacket:
    return CandidatePacket(
        candidate_id=cid,
        evidence_refs=("artifact:evidence-1",),
        candidate_type="graph_candidate",
        schema_version="universal-kb-candidate.v1",
        graph_node_refs=("node:paper:1", "node:claim:1"),
        graph_edge_refs=("edge:paper:1->claim:1",),
        provenance_refs=("source:arxiv:2605.18747",),
    )


def test_disabled_falkor_emits_falkor_specific_diagnostics() -> None:
    adapter = DisabledFalkorProjectionAdapter(dry_run=True)
    result = adapter.project(ProjectionRequest(candidate_packet=_packet()))
    codes = {d.code for d in result.diagnostics}
    assert "falkordb_no_write_translation" in codes
    assert "falkordb_dry_run_plan_ready" in codes
    assert result.backend == "falkordb"
    result.assert_no_write()


def test_dry_run_service_attaches_operation_plan() -> None:
    adapter = DisabledFalkorProjectionAdapter(dry_run=True)
    request = ProjectionRequest(candidate_packet=_packet())
    result, plan = project_with_falkor_plan(adapter, request)
    codes = [d.code for d in result.diagnostics]
    assert any(c.startswith("falkor_plan_fingerprint:") for c in codes)
    assert any(c == "falkor_writes_blocked" for c in codes)
    assert plan.plan_fingerprint
    assert all(not op.executable_now for op in plan.operations)
    assert any(op.kind == "write_blocked" for op in plan.operations)
    plan.assert_no_write()
    result.assert_no_write()


def test_operation_plan_idempotent_for_same_candidate() -> None:
    request = ProjectionRequest(candidate_packet=_packet("same-id"))
    p1 = build_falkor_operation_plan(request)
    p2 = build_falkor_operation_plan(request)
    assert p1.plan_fingerprint == p2.plan_fingerprint
    assert [o.op_id for o in p1.operations] == [o.op_id for o in p2.operations]
