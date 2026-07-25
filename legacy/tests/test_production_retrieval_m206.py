"""M206: production retrieval + graph quality over GraphReadPort."""

from __future__ import annotations

import ast
from datetime import UTC, datetime, timedelta
from pathlib import Path

import ladybug
import pytest

import research_graph.infrastructure.graph.ladybug_client as ladybug_client
from research_graph.application.graph.pilot_write_authorization import (
    issue_pilot_write_authorization,
)
from research_graph.application.graph.production_retrieval import (
    ReviewedQuery,
    decide_staged_query_gate,
    expand_queries_to_n,
    rehearse_read_degradation,
    run_integrity_audit,
    score_policy_on_queries,
    seed_and_lineage_parity,
    trace_one_query_evidence,
)
from research_graph.domain.ports import GraphReadPort
from research_graph.infrastructure.graph.graph_read_adapters import (
    LadybugGraphReadAdapter,
    SnapshotGraphReadAdapter,
)
from research_graph.infrastructure.graph.pilot_write import (
    DisposablePilotGraphStore,
    FalkorPilotGraphDBAdapter,
)
from research_graph.infrastructure.retrieval.hybrid import (
    HybridRetrievalMode,
    HybridRetrievalQuery,
    InMemoryVectorCandidateIndex,
    retrieve_hybrid,
)
from research_graph.workflows.rlm.graph_traversal import (
    RLMGraphTraversalConfig,
    RLMGraphTraversalQuestion,
    compare_rlm_graph_traversal,
)
from tests.test_ladybug_scientific_kg import build_fixture_payload

ROOT = Path(__file__).resolve().parents[1]
APP_MODULES = [
    ROOT / "src/research_graph/application/graph/production_retrieval.py",
]


@pytest.fixture()
def ladybug_conn() -> ladybug.Connection:
    db = ladybug.Database(":memory:")
    conn = ladybug.Connection(db)
    ladybug_client.init_scientific_kg_schema(conn)
    document, chunks, evidence_paths, patch = build_fixture_payload()
    ladybug_client.upsert_scientific_kg(conn, document, chunks, evidence_paths, patch)
    return conn


@pytest.fixture()
def falkor_snapshot() -> dict:
    document, chunks, evidence_paths, patch = build_fixture_payload()
    store = DisposablePilotGraphStore(store_id="m206-snap")
    now = datetime(2026, 7, 22, 12, 0, 0, tzinfo=UTC)
    auth = issue_pilot_write_authorization(
        auth_id="auth-m206",
        candidate_id="cand-m206",
        packet_hash="packet-m206",
        operation_plan_fingerprint="plan-m206",
        environment_prerequisites=(
            "graph_writes_allowed_explicit_true_in_future_milestone",
            "falkordb_write_driver_available",
        ),
        rollback_plan=("abort_before_commit_if_validation_fails",),
        expiry_utc=(now + timedelta(hours=2)).strftime("%Y-%m-%dT%H:%M:%SZ"),
        human_approval_token="human-m206",
        required_prerequisites=("falkordb_write_driver_available",),
        now=now,
    )
    adapter = FalkorPilotGraphDBAdapter(store, authorization=auth)
    adapter.init_schema()
    adapter.upsert_scientific_kg(document, chunks, evidence_paths, patch)
    return store.export_snapshot()


@pytest.fixture()
def vector_index() -> InMemoryVectorCandidateIndex:
    return InMemoryVectorCandidateIndex(
        {
            "2605.12345:method:chunk-0001": (1.0, 0.0, 0.0),
            "2605.12345:abstract:chunk-0001": (0.0, 1.0, 0.0),
            "2605.12345:conclusion:chunk-0001": (0.0, 0.0, 1.0),
        }
    )


def test_s01_backend_neutral_seed_lineage_shape(ladybug_conn, falkor_snapshot) -> None:
    lady = LadybugGraphReadAdapter(ladybug_conn)
    falk = SnapshotGraphReadAdapter(falkor_snapshot)
    assert isinstance(lady, GraphReadPort)
    assert isinstance(falk, GraphReadPort)
    parity = seed_and_lineage_parity(lady, falk, needle="PageIndex", limit=5)
    assert parity["seed_shape_compatible"] is True
    assert parity["lineage_shape_compatible"] is True
    # Typed keys present on any non-empty result
    for side in ("seed_ladybug", "seed_falkor", "lineage_ladybug", "lineage_falkor"):
        for row in parity[side]:
            assert set(row) >= {
                "semantic_chunk_id",
                "page_index_node_id",
                "evidence_path_id",
                "graph_score",
                "graph_source",
            }


def test_s02_retrieve_hybrid_on_falkor_snapshot(falkor_snapshot, vector_index) -> None:
    reader = SnapshotGraphReadAdapter(falkor_snapshot)
    response = retrieve_hybrid(
        query=HybridRetrievalQuery(
            text="method",
            vector=(1.0, 0.0, 0.0),
            mode=HybridRetrievalMode.HYBRID,
            limit=5,
        ),
        graph_read=reader,
        vector_index=vector_index,
    )
    assert response.results
    assert response.diagnostics["vector_candidate_count"] >= 1


def test_s02_retrieve_hybrid_legacy_conn_still_works(ladybug_conn, vector_index) -> None:
    response = retrieve_hybrid(
        ladybug_conn,
        HybridRetrievalQuery(
            text="PageIndex",
            vector=(1.0, 0.0, 0.0),
            mode=HybridRetrievalMode.HYBRID,
            limit=5,
        ),
        vector_index=vector_index,
    )
    assert response.results


def test_s03_one_query_evidence_trace(ladybug_conn, vector_index) -> None:
    reader = LadybugGraphReadAdapter(ladybug_conn)
    response = retrieve_hybrid(
        query=HybridRetrievalQuery(
            text="PageIndex",
            vector=(1.0, 0.0, 0.0),
            mode=HybridRetrievalMode.HYBRID,
            limit=5,
        ),
        graph_read=reader,
        vector_index=vector_index,
    )
    query = ReviewedQuery(
        query_id="q1",
        text="PageIndex",
        expected_chunk_ids=("2605.12345:method:chunk-0001",),
        query_vector=(1.0, 0.0, 0.0),
    )
    trace = trace_one_query_evidence(reader, query, ranked_rows=response.results)
    assert trace.source_chunk_ids
    assert trace.safety_flags.import_eligible is False
    assert "api_key" not in str(trace.to_dict())


def test_s04_baseline_harness_on_falkor_via_ladybug_parity(ladybug_conn, vector_index) -> None:
    # Existing compare_rlm_graph_traversal reuses hybrid; still works on Ladybug fixture.
    question = RLMGraphTraversalQuestion(
        name="q-baseline",
        query="PageIndex",
        query_vector=(1.0, 0.0, 0.0),
        seed_semantic_chunk_ids=("2605.12345:method:chunk-0001",),
        expected_semantic_chunk_ids=frozenset({"2605.12345:method:chunk-0001"}),
    )
    result = compare_rlm_graph_traversal(
        ladybug_conn,
        question,
        vector_index=vector_index,
        config=RLMGraphTraversalConfig(max_steps=2, max_hops=2, top_k=4),
    )
    labels = {b.label for b in result.baselines}
    assert labels >= {"vector_only", "graph_one_hop", "hybrid", "heuristic_bfs"}


def test_s05_rlm_budgets_config() -> None:
    cfg = RLMGraphTraversalConfig(max_hops=2, max_query_tokens=128, max_wall_clock_ms=1000)
    assert cfg.max_hops == 2
    assert cfg.max_steps == 2  # hop budget clamps steps
    assert cfg.max_query_tokens == 128
    assert cfg.max_wall_clock_ms == 1000


def test_s06_retrieval_quality_benchmark(ladybug_conn, vector_index) -> None:
    reader = LadybugGraphReadAdapter(ladybug_conn)
    queries = [
        ReviewedQuery(
            query_id="q1",
            text="PageIndex",
            expected_chunk_ids=("2605.12345:method:chunk-0001",),
            query_vector=(1.0, 0.0, 0.0),
        )
    ]

    def hybrid_retrieve(q: ReviewedQuery):
        resp = retrieve_hybrid(
            query=HybridRetrievalQuery(
                text=q.text,
                vector=q.query_vector,
                mode=HybridRetrievalMode.HYBRID,
                limit=5,
            ),
            graph_read=reader,
            vector_index=vector_index,
        )
        return resp.results

    metrics = score_policy_on_queries(
        policy="hybrid",
        queries=queries,
        retrieve_fn=hybrid_retrieve,
        latency_ms=2.0,
    )
    assert metrics.recall > 0
    assert metrics.failure_rate == 0
    assert metrics.query_count == 1


def test_s07_graph_integrity_audit(ladybug_conn, falkor_snapshot) -> None:
    lady = run_integrity_audit(LadybugGraphReadAdapter(ladybug_conn))
    falk = run_integrity_audit(SnapshotGraphReadAdapter(falkor_snapshot))
    assert lady.broken_evidence_paths == 0
    assert falk.broken_evidence_paths == 0
    assert lady.schema_violations == 0
    assert falk.backend == "falkor_snapshot"


def test_s08_read_failure_degradation() -> None:
    out = rehearse_read_degradation(
        scenario="backend_unavailable",
        allow_vector_only_fallback=True,
        vector_only_ok=True,
    )
    assert out.safe is True
    assert out.degraded_to == "vector_only"
    malformed = rehearse_read_degradation(scenario="malformed_query")
    assert malformed.safe is True
    assert malformed.degraded_to is None


def test_s09_ten_and_twenty_query_gate(ladybug_conn, vector_index) -> None:
    reader = LadybugGraphReadAdapter(ladybug_conn)
    base = [
        ReviewedQuery(
            query_id="q1",
            text="PageIndex",
            expected_chunk_ids=("2605.12345:method:chunk-0001",),
            query_vector=(1.0, 0.0, 0.0),
        )
    ]
    for n in (10, 20):
        queries = expand_queries_to_n(base, n)

        def hybrid_retrieve(q: ReviewedQuery, _reader=reader):
            resp = retrieve_hybrid(
                query=HybridRetrievalQuery(
                    text=q.text,
                    vector=q.query_vector,
                    mode=HybridRetrievalMode.HYBRID,
                    limit=5,
                ),
                graph_read=_reader,
                vector_index=vector_index,
            )
            return resp.results

        metrics = score_policy_on_queries(
            policy="hybrid",
            queries=queries,
            retrieve_fn=hybrid_retrieve,
        )
        integrity = run_integrity_audit(reader)
        gate = decide_staged_query_gate(
            query_count=n,
            policy_metrics=(metrics,),
            integrity=integrity,
        )
        assert gate.query_count == n
        assert gate.verdict in {"proceed", "repair", "stop"}
        assert gate.verdict == "proceed"
        assert gate.safety_flags.import_eligible is False


def test_s10_ownership_ratchet_application_no_driver_imports() -> None:
    forbidden = {"ladybug", "falkordb", "redis"}
    for path in APP_MODULES:
        tree = ast.parse(path.read_text(encoding="utf-8"))
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    assert alias.name.split(".")[0] not in forbidden
            if isinstance(node, ast.ImportFrom) and node.module:
                root = node.module.split(".")[0]
                assert root not in forbidden
                assert not node.module.startswith("research_graph.infrastructure")
    # hybrid must not import ladybug package directly anymore
    hybrid_src = (ROOT / "src/research_graph/infrastructure/retrieval/hybrid.py").read_text()
    assert "import ladybug" not in hybrid_src
    # production retrieval must not reimplement retrieve_hybrid fusion
    prod = (ROOT / "src/research_graph/application/graph/production_retrieval.py").read_text()
    assert "def retrieve_hybrid" not in prod
