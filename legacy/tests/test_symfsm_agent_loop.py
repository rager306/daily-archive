"""M208: read-only SymFSM agent loop and O1–O6 operators."""

from __future__ import annotations

from pathlib import Path

import ladybug
import pytest

import research_graph.infrastructure.graph.ladybug_client as ladybug_client
from research_graph.workflows.composition.symfsm_loop import (
    ALLOWED_TRANSITIONS,
    TERMINAL_STATES,
    agent_capability_ratchet,
    rehearse_adversarial_scenarios,
    run_read_only_symfsm_loop,
    verify_agent_output,
)
from research_graph.workflows.composition.symfsm_operators import (
    ALLOWLISTED_OPERATORS,
    assert_operator_allowlisted,
    o1_resolve_seed,
    o2_citation_lineage,
    o3_method_neighborhood,
    o4_topic_neighborhood,
    o5_gap_detection,
    o6_related_source_discovery,
)
from research_graph.infrastructure.graph.graph_read_adapters import LadybugGraphReadAdapter
from research_graph.infrastructure.retrieval.hybrid import InMemoryVectorCandidateIndex
from tests.test_ladybug_scientific_kg import build_fixture_payload

ROOT = Path(__file__).resolve().parents[1]
OP_MOD = ROOT / "src/research_graph/workflows/composition/symfsm_operators.py"
LOOP_MOD = ROOT / "src/research_graph/workflows/composition/symfsm_loop.py"


@pytest.fixture()
def graph_read() -> LadybugGraphReadAdapter:
    db = ladybug.Database(":memory:")
    conn = ladybug.Connection(db)
    ladybug_client.init_scientific_kg_schema(conn)
    document, chunks, evidence_paths, patch = build_fixture_payload()
    ladybug_client.upsert_scientific_kg(conn, document, chunks, evidence_paths, patch)
    return LadybugGraphReadAdapter(conn)


@pytest.fixture()
def vector_index() -> InMemoryVectorCandidateIndex:
    return InMemoryVectorCandidateIndex(
        {
            "2605.12345:method:chunk-0001": (1.0, 0.0, 0.0),
            "2605.12345:abstract:chunk-0001": (0.0, 1.0, 0.0),
        }
    )


def test_s01_o1_seed_resolution(graph_read, vector_index) -> None:
    result = o1_resolve_seed(
        graph_read,
        "PageIndex",
        limit=5,
        vector_index=vector_index,
        query_vector=(1.0, 0.0, 0.0),
    )
    assert result.operator == "O1"
    assert result.bounded is True
    assert result.safety_flags.import_eligible is False
    assert result.refs or result.ambiguity  # match or explicit no_match
    if len(result.refs) > 1:
        assert any(a.startswith("multi_match") for a in result.ambiguity)


def test_s01_empty_seed_ambiguity(graph_read) -> None:
    result = o1_resolve_seed(graph_read, "   ")
    assert "empty_seed" in result.ambiguity


def test_s02_o2_citation_lineage(graph_read, vector_index) -> None:
    seed = o1_resolve_seed(
        graph_read, "PageIndex", vector_index=vector_index, query_vector=(1.0, 0.0, 0.0)
    )
    assert seed.refs
    lineage = o2_citation_lineage(graph_read, seed.refs[0], limit=8)
    assert lineage.operator == "O2"
    assert lineage.bounded is True
    assert lineage.limit == 8


def test_s03_o3_o4_neighborhoods(graph_read, vector_index) -> None:
    seed = o1_resolve_seed(
        graph_read, "PageIndex", vector_index=vector_index, query_vector=(1.0, 0.0, 0.0)
    )
    assert seed.refs
    o3 = o3_method_neighborhood(graph_read, seed.refs[0], limit=8)
    o4 = o4_topic_neighborhood(graph_read, "PageIndex", limit=8)
    assert o3.operator == "O3"
    assert o4.operator == "O4"
    assert any("source_diversity" in d for d in o3.diagnostics + o4.diagnostics)


def test_s04_o5_gap_detection(graph_read, vector_index) -> None:
    seed = o1_resolve_seed(
        graph_read, "PageIndex", vector_index=vector_index, query_vector=(1.0, 0.0, 0.0)
    )
    o2 = o2_citation_lineage(graph_read, seed.refs[0] if seed.refs else "PageIndex")
    gaps = o5_gap_detection(
        graph_read,
        tuple(seed.refs) + tuple(o2.refs),
        expected_chunk_ids=("missing:expected:chunk",),
    )
    assert gaps.operator == "O5"
    assert any(r.ref_type == "gap" for r in gaps.refs)
    assert any("missing_expected" in d for d in gaps.diagnostics)


def test_s05_o6_related_source_discovery(graph_read, vector_index) -> None:
    gaps = o5_gap_detection(
        graph_read,
        (),
        expected_chunk_ids=("2605.12345:method:chunk-0001",),
    )
    suggestions = o6_related_source_discovery(
        graph_read,
        gaps.refs,
        vector_index=vector_index,
        query_vector=(1.0, 0.0, 0.0),
        limit=5,
    )
    assert suggestions.operator == "O6"
    assert suggestions.bounded is True
    assert len(suggestions.refs) <= 5


def test_s06_finite_state_loop(graph_read, vector_index) -> None:
    trace = run_read_only_symfsm_loop(
        graph_read,
        seed="PageIndex",
        topic="PageIndex",
        expected_chunk_ids=("2605.12345:method:chunk-0001",),
        vector_index=vector_index,
        query_vector=(1.0, 0.0, 0.0),
    )
    assert trace.states[0] == "RESOLVE"
    assert trace.terminal in TERMINAL_STATES
    assert set(trace.operators_called) <= set(ALLOWLISTED_OPERATORS)
    # legal transitions
    for a, b in zip(trace.states, trace.states[1:], strict=False):
        if a in TERMINAL_STATES:
            break
        assert b in ALLOWED_TRANSITIONS[a] or b == "FAILED"
    assert trace.safety_flags.import_eligible is False


def test_s07_cognitive_map_and_repair_not_applied(graph_read, vector_index) -> None:
    trace = run_read_only_symfsm_loop(
        graph_read,
        seed="PageIndex",
        expected_chunk_ids=("missing:chunk:x",),
        vector_index=vector_index,
        query_vector=(1.0, 0.0, 0.0),
    )
    assert trace.cognitive_map is not None
    assert trace.cognitive_map.claims
    if trace.repair is not None:
        assert trace.repair.applied is False


def test_s08_output_verification_gate() -> None:
    # reject unknown tool
    bad = verify_agent_output(
        map_=None,
        repair=None,
        operators_called=("O1", "write_graph"),
        terminal="DONE",
        tools_requested=("write_graph",),
    )
    assert bad.accepted is False
    assert any("tool" in r for r in bad.rejected_reasons)

    # reject incomplete terminal
    incomplete = verify_agent_output(
        map_=None,
        repair=None,
        operators_called=("O1",),
        terminal="RESOLVE",  # type: ignore[arg-type]
    )
    assert incomplete.accepted is False


def test_s09_adversarial_and_outage_uat(graph_read, vector_index) -> None:
    outcomes = rehearse_adversarial_scenarios(
        graph_read,
        seed="PageIndex",
        vector_index=vector_index,
        query_vector=(1.0, 0.0, 0.0),
    )
    scenarios = {o.scenario for o in outcomes}
    assert scenarios >= {
        "prompt_injection",
        "cyclic_graph",
        "oversized_neighborhood",
        "backend_outage",
        "provider_outage",
    }
    assert all(o.safe for o in outcomes)
    assert all(o.terminal in TERMINAL_STATES for o in outcomes)


def test_s10_capability_ratchet_and_verdict(graph_read, vector_index) -> None:
    trace = run_read_only_symfsm_loop(
        graph_read,
        seed="PageIndex",
        vector_index=vector_index,
        query_vector=(1.0, 0.0, 0.0),
    )
    adv = rehearse_adversarial_scenarios(
        graph_read, seed="PageIndex", vector_index=vector_index, query_vector=(1.0, 0.0, 0.0)
    )
    verdict = agent_capability_ratchet(
        module_paths=[OP_MOD, LOOP_MOD],
        adversarial=adv,
        last_trace=trace,
    )
    assert verdict.allowlisted_operators == ALLOWLISTED_OPERATORS
    assert verdict.verdict in {"proceed", "repair", "stop"}
    # no real write calls in modules
    assert verdict.forbidden_found == ()
    if trace.terminal == "DONE" and trace.verifier and trace.verifier.accepted:
        assert verdict.verdict == "proceed"


def test_operator_allowlist_rejects_forbidden() -> None:
    with pytest.raises(PermissionError):
        assert_operator_allowlisted("write_graph")
    for op in ALLOWLISTED_OPERATORS:
        assert_operator_allowlisted(op)


def test_repair_cannot_be_applied() -> None:
    from research_graph.workflows.composition.symfsm_loop import RepairSuggestion

    with pytest.raises(ValueError, match="must not be applied"):
        RepairSuggestion(gap_ref_id="g1", suggested_source_refs=(), applied=True)
