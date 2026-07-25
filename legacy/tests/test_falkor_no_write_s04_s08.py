"""M203 S04–S08: parity, idempotency, schema rehearsal, query plans, verdict."""

from __future__ import annotations

from research_graph.application.graph.falkor_capability import probe_falkor_capabilities
from research_graph.application.graph.falkor_no_write_verdict import (
    CONTROLLED_WRITE_PILOT_PREREQUISITES,
    decide_no_write_backend_verdict,
)
from research_graph.application.graph.falkor_operation_plan import build_falkor_operation_plan
from research_graph.application.graph.falkor_parity import compare_projection_parity
from research_graph.application.graph.falkor_projection_adequacy import (
    assess_projection_port_adequacy,
)
from research_graph.application.graph.falkor_query_plans import build_seed_and_lineage_plans
from research_graph.application.graph.falkor_schema_rehearsal import rehearse_schema_evolution
from research_graph.domain.ports import ProjectionRequest
from research_graph.infrastructure.graph.networkx_probe import NetworkXProjectionAdapter
from research_graph.infrastructure.graph.projection_backends import (
    DisabledFalkorProjectionAdapter,
)
from research_graph.workflows.universal_kb.contracts import CandidatePacket


def _packet(cid: str = "candidate-parity-1") -> CandidatePacket:
    return CandidatePacket(
        candidate_id=cid,
        evidence_refs=("artifact:evidence-1",),
        candidate_type="graph_candidate",
        schema_version="universal-kb-candidate.v1",
        graph_node_refs=("node:paper:1", "node:claim:1"),
        graph_edge_refs=("edge:paper:1->claim:1",),
        provenance_refs=("source:arxiv:2605.18747",),
    )


def test_s04_networkx_falkor_dry_run_parity_match() -> None:
    request = ProjectionRequest(candidate_packet=_packet())
    report = compare_projection_parity(
        request,
        networkx_adapter=NetworkXProjectionAdapter(),
        falkor_adapter=DisabledFalkorProjectionAdapter(dry_run=True),
    )
    assert report.verdict == "match"
    assert "node:paper:1" in report.shared_node_refs
    assert "edge:paper:1->claim:1" in report.shared_edge_refs
    report.assert_no_write()


def test_s05_operation_plan_idempotent_and_phases() -> None:
    request = ProjectionRequest(candidate_packet=_packet("idem-1"))
    p1 = build_falkor_operation_plan(request)
    p2 = build_falkor_operation_plan(request)
    assert p1.plan_fingerprint == p2.plan_fingerprint
    assert p1.transaction_phases == (
        "prepare",
        "validate",
        "execute_deferred",
        "commit_deferred",
    )
    assert all(not op.executable_now for op in p1.operations)
    assert "execute_deferred" in p1.transaction_phases
    p1.assert_no_write()


def test_s06_schema_rehearsal_migration_placeholder() -> None:
    request = ProjectionRequest(candidate_packet=_packet())
    report = rehearse_schema_evolution(request)
    assert report.current.accepted is True
    assert report.next_version.migration_required is True
    assert report.verdict == "migration_placeholder"
    assert report.next_version.migration_plan is not None
    assert report.next_version.migration_plan.status == "placeholder_only"
    report.assert_no_write()


def test_s07_seed_and_lineage_query_plans() -> None:
    request = ProjectionRequest(candidate_packet=_packet())
    bundle = build_seed_and_lineage_plans(request)
    assert bundle.o1_seed.kind == "O1_seed"
    assert bundle.o2_lineage.kind == "O2_lineage"
    assert bundle.o1_seed.validated is True
    assert bundle.o2_lineage.validated is True
    assert all(not op.executable_now for op in bundle.o1_seed.operations)
    assert all(not op.executable_now for op in bundle.o2_lineage.operations)
    bundle.assert_no_write()


def test_s08_no_write_backend_verdict_proceed() -> None:
    request = ProjectionRequest(candidate_packet=_packet())
    capability = probe_falkor_capabilities()
    adequacy = assess_projection_port_adequacy(
        request,
        capability=capability,
        sample_result=DisabledFalkorProjectionAdapter(dry_run=True).project(request),
    )
    parity = compare_projection_parity(
        request,
        networkx_adapter=NetworkXProjectionAdapter(),
        falkor_adapter=DisabledFalkorProjectionAdapter(dry_run=True),
    )
    schema = rehearse_schema_evolution(request)
    queries = build_seed_and_lineage_plans(request)
    verdict = decide_no_write_backend_verdict(
        capability=capability,
        adequacy=adequacy,
        parity=parity,
        schema=schema,
        queries=queries,
    )
    assert verdict.verdict == "proceed"
    assert "controlled_write_adapter_still_required" in verdict.reasons
    assert "GraphDBPort_adapter_implementing_upsert_scientific_kg" in CONTROLLED_WRITE_PILOT_PREREQUISITES
    assert "write_driver_falkordb_or_redis_protocol" in verdict.controlled_write_pilot_prerequisites
    verdict.assert_no_write()
    payload = str(verdict.to_dict()).lower()
    assert "api_key" not in payload
    assert "embedding" not in payload
