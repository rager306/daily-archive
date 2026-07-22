"""M204: explicit promotion boundary composition tests."""

from __future__ import annotations

import ast
from datetime import UTC, datetime
from pathlib import Path

import pytest

from research_graph.application.graph.promotion_boundary import (
    build_pilot_approval_packet,
    compact_operator_packet,
    decide_pilot_eligibility,
    observe_import_boundary,
    observe_readiness_handoff,
    observe_review_post_check,
    observe_schema_gate,
    trace_promotion_boundary_gaps,
)
from research_graph.domain.graph_projection_schema import (
    GraphProjectionSchemaGate,
    SchemaGateResult,
    SchemaMigrationPlan,
)
from research_graph.domain.ports import ProjectionRequest
from research_graph.workflows.universal_kb.contracts import CandidatePacket

APP = Path("src/research_graph/application/graph/promotion_boundary.py")
OWNERSHIP_TARGETS = [
    Path("src/research_graph/application/chunk_extraction.py"),
    Path("src/research_graph/application/paper_extraction.py"),
    Path("src/research_graph/application/extraction_pilot.py"),
    Path("src/research_graph/infrastructure/graph/projection_backends.py"),
    Path("src/research_graph/infrastructure/staging/import_boundary.py"),
    Path("src/research_graph/application/graph/promotion_boundary.py"),
]
FORBIDDEN_WRITE_CALLS = {
    "upsert_scientific_kg",
    "init_schema",
}


def _packet(cid: str = "candidate-promo-1") -> CandidatePacket:
    return CandidatePacket(
        candidate_id=cid,
        evidence_refs=("artifact:evidence-1",),
        candidate_type="graph_candidate",
        schema_version="universal-kb-candidate.v1",
        graph_node_refs=("node:paper:1",),
        graph_edge_refs=("edge:paper:1->claim:1",),
        provenance_refs=("source:arxiv:2605.18747",),
    )


def _passing_seams(cid: str = "candidate-promo-1"):
    request = ProjectionRequest(candidate_packet=_packet(cid))
    schema = observe_schema_gate(GraphProjectionSchemaGate().validate(request))
    return (
        observe_review_post_check(completed=True),
        observe_import_boundary(valid_rehearsal=True, accepted_count=0),
        schema,
        observe_readiness_handoff(readiness_state="diagnostics_only"),
    )


def test_s01_gap_report_all_seams_pass() -> None:
    review, boundary, schema, handoff = _passing_seams()
    report = trace_promotion_boundary_gaps(
        candidate_id="candidate-promo-1",
        review=review,
        import_boundary=boundary,
        schema=schema,
        handoff=handoff,
    )
    assert report.review_post_check_first is True
    assert report.import_eligible is False
    assert report.graph_write_allowed is False
    assert report.has_blockers is False
    assert [s.name for s in report.seams][0] == "review_post_check"
    report.assert_no_write()


def test_s01_review_incomplete_is_blocker() -> None:
    review = observe_review_post_check(completed=False)
    _, boundary, schema, handoff = _passing_seams()
    report = trace_promotion_boundary_gaps(
        candidate_id="candidate-promo-1",
        review=review,
        import_boundary=boundary,
        schema=schema,
        handoff=handoff,
    )
    assert report.has_blockers is True
    assert any(g.severity == "blocker" for g in report.gaps)


def test_s01_import_eligible_true_fails_boundary() -> None:
    obs = observe_import_boundary(
        valid_rehearsal=True, accepted_count=0, import_eligible_any=True
    )
    assert obs.passed is False


def test_s02_pilot_eligibility_eligible() -> None:
    review, boundary, schema, handoff = _passing_seams()
    gaps = trace_promotion_boundary_gaps(
        candidate_id="candidate-promo-1",
        review=review,
        import_boundary=boundary,
        schema=schema,
        handoff=handoff,
    )
    decision = decide_pilot_eligibility(gaps)
    assert decision.decision == "eligible"
    assert decision.pilot_eligible is True
    assert decision.import_eligible is False
    assert decision.graph_write_allowed is False
    assert decision.persistence_authority is False
    assert decision.graph_adapter_invocations == 0
    decision.assert_no_write()


def test_s02_pilot_eligibility_denied_on_review_gap() -> None:
    review = observe_review_post_check(completed=False)
    _, boundary, schema, handoff = _passing_seams()
    gaps = trace_promotion_boundary_gaps(
        candidate_id="candidate-promo-1",
        review=review,
        import_boundary=boundary,
        schema=schema,
        handoff=handoff,
    )
    decision = decide_pilot_eligibility(gaps)
    assert decision.decision == "denied"
    assert decision.pilot_eligible is False


def test_s03_positive_no_write_rehearsal_zero_adapter_invocations() -> None:
    review, boundary, schema, handoff = _passing_seams("safe-reviewed-1")
    gaps = trace_promotion_boundary_gaps(
        candidate_id="safe-reviewed-1",
        review=review,
        import_boundary=boundary,
        schema=schema,
        handoff=handoff,
    )
    decision = decide_pilot_eligibility(gaps)
    assert decision.pilot_eligible is True
    assert decision.graph_adapter_invocations == 0
    # schema gate accepted without migration
    assert schema.passed is True


def test_s04_and_s06_approval_packet_hash_and_compact() -> None:
    review, boundary, schema, handoff = _passing_seams()
    gaps = trace_promotion_boundary_gaps(
        candidate_id="candidate-promo-1",
        review=review,
        import_boundary=boundary,
        schema=schema,
        handoff=handoff,
    )
    decision = decide_pilot_eligibility(gaps)
    packet = build_pilot_approval_packet(
        decision,
        operation_plan_fingerprint="deadbeefcafebabe",
        now=datetime(2026, 7, 22, 12, 0, 0, tzinfo=UTC),
    )
    assert packet.packet_hash
    assert packet.expiry_utc.startswith("2026-07-25")
    assert packet.import_eligible is False
    assert packet.graph_write_allowed is False
    assert "GraphDBPort_adapter_path_m205_only" in packet.environment_prerequisites
    compact = compact_operator_packet(packet)
    assert compact["packet_hash"] == packet.packet_hash
    assert compact["import_eligible"] is False
    assert "api_key" not in str(compact).lower()
    packet.assert_no_write()


def test_s04_approval_requires_eligible() -> None:
    review = observe_review_post_check(completed=False)
    _, boundary, schema, handoff = _passing_seams()
    gaps = trace_promotion_boundary_gaps(
        candidate_id="candidate-promo-1",
        review=review,
        import_boundary=boundary,
        schema=schema,
        handoff=handoff,
    )
    decision = decide_pilot_eligibility(gaps)
    with pytest.raises(ValueError, match="pilot_eligible"):
        build_pilot_approval_packet(decision, operation_plan_fingerprint="x")


def test_schema_gate_migration_not_passed() -> None:
    result = SchemaGateResult(
        candidate_schema_version="universal-kb-candidate.v1",
        projection_schema_version="graph-projection.v0-next",
        accepted=False,
        migration_required=True,
        diagnostics=("schema_migration_required",),
        migration_plan=SchemaMigrationPlan(
            from_schema_version="graph-projection.v0-next",
            to_schema_version="knowledge-graph-projection.v1",
        ),
    )
    obs = observe_schema_gate(result)
    assert obs.passed is False


def _call_names(path: Path) -> set[str]:
    tree = ast.parse(path.read_text(encoding="utf-8"))
    names: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Call):
            func = node.func
            if isinstance(func, ast.Name):
                names.add(func.id)
            elif isinstance(func, ast.Attribute):
                names.add(func.attr)
    return names


def test_s05_s07_ownership_ratchet_no_direct_graphdb_writes() -> None:
    for path in OWNERSHIP_TARGETS:
        assert path.exists(), path
        calls = _call_names(path)
        leaked = calls & FORBIDDEN_WRITE_CALLS
        assert not leaked, f"{path} must not call {leaked}"
    # promotion_boundary must not import infrastructure GraphDB adapters
    src = APP.read_text(encoding="utf-8")
    assert "research_graph.infrastructure" not in src
    assert "GraphDBPort" not in src or "GraphDBPort_adapter_path_m205_only" in src
    assert "upsert_scientific_kg" not in src


def test_application_does_not_import_infrastructure() -> None:
    roots: set[str] = set()
    tree = ast.parse(APP.read_text(encoding="utf-8"))
    for node in ast.walk(tree):
        if isinstance(node, ast.ImportFrom) and node.module:
            roots.add(node.module)
        elif isinstance(node, ast.Import):
            for alias in node.names:
                roots.add(alias.name)
    assert not any(r.startswith("research_graph.infrastructure") for r in roots)
