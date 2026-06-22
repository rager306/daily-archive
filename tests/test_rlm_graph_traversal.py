"""Contract tests for the S10 deterministic RLM graph traversal spike.

These tests define a fixture-level traversal boundary that compares an
RLM-style graph traversal policy against vector-only retrieval, one-hop graph
expansion, hybrid retrieval, and heuristic BFS baselines. The contract is
intentionally deterministic: no DSPy optimizer, live RLM runtime, embedding
service, network client, or LadybugDB write helper may be imported or invoked.
Diagnostics and reprs must remain text-safe: IDs, counts, modes, statuses, stop
reasons, and numeric scores only.
"""

from __future__ import annotations

import ast
from copy import deepcopy
from dataclasses import dataclass
from pathlib import Path
from typing import Any, cast

import ladybug
import pytest

import research_graph.infrastructure.graph.ladybug_client as ladybug_client
from research_graph.domain.semantic_chunks import EvidencePath
from research_graph.infrastructure.graph.ladybug_client import evidence_path_id
from tests.test_ladybug_scientific_kg import build_fixture_patch
from tests.test_rlm_workflow import build_valid_inputs

RLM_GRAPH_TRAVERSAL_MODULE = Path("src/research_graph/workflows/rlm/graph_traversal.py")
RAW_FIXTURE_BODY_TEXT = (
    "The agent builds a PageIndex from deterministic local markdown before any network or "
    "PDF extraction is attempted."
)
RAW_FIXTURE_CLAIM_TEXT = "Local markdown is enough to build a deterministic PageIndex."
RAW_OPTIMIZER_TEXT = "MIPRO optimizer trace should never appear"
RAW_VECTOR_PAYLOAD = "[0.98, 0.01, 0.00, 0.42]"

FORBIDDEN_IMPORT_ROOTS = {
    "dspy",
    "socket",
    "httpx",
    "requests",
    "urllib",
    "openai",
    "anthropic",
    "cohere",
    "sentence_transformers",
    "transformers",
    "subprocess",
    "sqlite3",
}
FORBIDDEN_RUNTIME_REFERENCES = {
    "dspy",
    "teleprompt",
    "MIPRO",
    "MIPROv2",
    "GEPA",
    "BootstrapFewShot",
    "BootstrapFewShotWithRandomSearch",
    "socket",
    "create_connection",
    "HTTPConnection",
    "HTTPSConnection",
    "httpx",
    "requests",
    "urlopen",
    "OpenAI",
    "Anthropic",
    "Cohere",
    "SentenceTransformer",
    "Embedding",
    "embeddings",
    "Database",
    "Connection",
    "connect",
    "execute_write",
    "executemany",
    "commit",
    "upsert_scientific_kg",
    "init_db",
    "init_scientific_kg_schema",
    "CREATE",
    "MERGE",
    "SET",
    "DELETE",
    "subprocess",
    "Popen",
    "run",
    "call",
    "check_call",
    "check_output",
    "system",
    "popen",
}


@dataclass(frozen=True)
class FixtureVector:
    """Deterministic test-only vector attached to one SemanticChunk."""

    semantic_chunk_id: str
    values: tuple[float, ...]


@dataclass(frozen=True)
class ScatteredFixture:
    """Fixture graph with evidence expected from different PageIndex nodes."""

    conn: ladybug.Connection
    vectors: list[FixtureVector]
    expected_semantic_chunk_ids: frozenset[str]
    expected_evidence_path_ids: frozenset[str]
    seed_semantic_chunk_ids: tuple[str, ...]
    seed_evidence_path_ids: tuple[str, ...]
    query: str
    query_vector: tuple[float, ...]


@pytest.fixture()
def scattered_fixture() -> ScatteredFixture:
    """Persist deterministic PageIndex, chunks, EvidencePaths, and one KG patch."""
    db = ladybug.Database(":memory:")
    conn = ladybug.Connection(db)
    ladybug_client.init_scientific_kg_schema(conn)
    document, chunks, evidence_paths = build_valid_inputs()

    method_chunk = next(
        chunk for chunk in chunks if chunk.page_index_node_id == "2605.12345:method"
    )
    conclusion_chunk = next(
        chunk for chunk in chunks if chunk.page_index_node_id == "2605.12345:conclusion"
    )
    method_evidence = _evidence_for_chunk(evidence_paths, method_chunk.id)
    conclusion_evidence = _evidence_for_chunk(evidence_paths, conclusion_chunk.id)

    ladybug_client.upsert_scientific_kg(
        conn,
        document,
        chunks,
        evidence_paths,
        build_fixture_patch(method_evidence),
    )

    return ScatteredFixture(
        conn=conn,
        vectors=[
            FixtureVector(method_chunk.id, (1.0, 0.0, 0.0, 0.0)),
            FixtureVector("2605.12345:abstract:chunk-0001", (0.87, 0.0, 0.0, 0.0)),
            FixtureVector(conclusion_chunk.id, (0.0, 1.0, 0.0, 0.0)),
            FixtureVector("2605.12345:introduction:chunk-0001", (0.25, 0.25, 0.0, 0.0)),
        ],
        expected_semantic_chunk_ids=frozenset({method_chunk.id, conclusion_chunk.id}),
        expected_evidence_path_ids=frozenset(
            {evidence_path_id(method_evidence), evidence_path_id(conclusion_evidence)}
        ),
        seed_semantic_chunk_ids=(method_chunk.id,),
        seed_evidence_path_ids=(evidence_path_id(method_evidence),),
        query="Which evidence connects deterministic PageIndex construction to the paper outcome?",
        query_vector=(0.82, 0.82, 0.0, 0.0),
    )


def _evidence_for_chunk(evidence_paths: list[EvidencePath], semantic_chunk_id: str) -> EvidencePath:
    return next(path for path in evidence_paths if path.semantic_chunk_id == semantic_chunk_id)


def _as_mapping(value: Any) -> dict[str, Any]:
    if isinstance(value, dict):
        return value
    return value.__dict__


def test_public_contract_compares_rlm_graph_traversal_against_all_baselines(
    scattered_fixture: ScatteredFixture,
) -> None:
    """Facade returns typed RLM trajectory plus per-baseline ID-only metrics."""
    from research_graph.infrastructure.retrieval.hybrid import (
        InMemoryVectorCandidateIndex,  # noqa: PLC0415
    )
    from research_graph.workflows.rlm.graph_traversal import (  # noqa: PLC0415 - planned S10 API
        RLMGraphTraversalConfig,
        RLMGraphTraversalPolicy,
        RLMGraphTraversalQuestion,
        TraversalPolicyLabel,
        compare_rlm_graph_traversal,
    )

    result = compare_rlm_graph_traversal(
        scattered_fixture.conn,
        RLMGraphTraversalQuestion(
            name="scattered-pageindex-outcome",
            query=scattered_fixture.query,
            query_vector=scattered_fixture.query_vector,
            seed_semantic_chunk_ids=scattered_fixture.seed_semantic_chunk_ids,
            seed_evidence_path_ids=scattered_fixture.seed_evidence_path_ids,
            expected_semantic_chunk_ids=scattered_fixture.expected_semantic_chunk_ids,
            expected_evidence_path_ids=scattered_fixture.expected_evidence_path_ids,
        ),
        vector_index=InMemoryVectorCandidateIndex.from_fixture_vectors(scattered_fixture.vectors),
        config=RLMGraphTraversalConfig(max_steps=4, max_neighbors_per_step=3, top_k=4),
        policy=RLMGraphTraversalPolicy(label=TraversalPolicyLabel.RLM_STYLE_DETERMINISTIC),
    )

    assert result.question_id == "scattered-pageindex-outcome"
    assert result.config.max_steps == 4
    assert result.config.max_neighbors_per_step == 3
    assert result.config.top_k == 4
    assert result.rlm_traversal.policy_label == "rlm_style_deterministic"
    assert result.rlm_traversal.stop_reason in {"target_recall_reached", "budget_exhausted"}
    assert (
        result.rlm_traversal.visited_semantic_chunk_ids[0]
        in scattered_fixture.seed_semantic_chunk_ids
    )
    assert (
        set(result.rlm_traversal.returned_semantic_chunk_ids)
        >= scattered_fixture.expected_semantic_chunk_ids
    )
    assert (
        set(result.rlm_traversal.returned_evidence_path_ids)
        >= scattered_fixture.expected_evidence_path_ids
    )
    assert result.rlm_traversal.metrics.retrieval_recall == pytest.approx(1.0)
    assert result.rlm_traversal.metrics.evidence_path_hit_rate == pytest.approx(1.0)
    assert result.rlm_traversal.metrics.missing_expected_result_ids == []
    assert result.rlm_traversal.metrics.missing_expected_evidence_path_ids == []
    assert result.rlm_traversal.budget_exhausted is False

    steps = [_as_mapping(step) for step in result.rlm_traversal.trajectory]
    assert steps
    assert {"step_index", "from_id", "to_id", "action", "score", "status"} <= set(steps[0])
    assert all(isinstance(step["step_index"], int) for step in steps)
    assert all(
        step["action"] in {"seed", "expand_neighbor", "select_candidate", "stop"} for step in steps
    )

    by_label = {baseline.label: baseline for baseline in result.baselines}
    assert set(by_label) == {"vector_only", "graph_one_hop", "hybrid", "heuristic_bfs"}
    for label, baseline in by_label.items():
        assert baseline.question_id == "scattered-pageindex-outcome"
        assert baseline.label == label
        assert baseline.returned_semantic_chunk_ids == sorted(
            set(baseline.returned_semantic_chunk_ids)
        )
        assert baseline.returned_evidence_path_ids == sorted(
            set(baseline.returned_evidence_path_ids)
        )
        assert 0.0 <= baseline.metrics.retrieval_recall <= 1.0
        assert 0.0 <= baseline.metrics.evidence_path_hit_rate <= 1.0
        assert isinstance(baseline.metrics.missing_expected_result_ids, list)
        assert isinstance(baseline.metrics.missing_expected_evidence_path_ids, list)
        assert isinstance(baseline.source_diagnostics, dict)

    assert by_label["vector_only"].source_diagnostics["mode"] == "vector_only"
    assert by_label["graph_one_hop"].source_diagnostics["mode"] in {"graph_only", "graph_one_hop"}
    assert by_label["hybrid"].source_diagnostics["mode"] == "hybrid"
    assert by_label["heuristic_bfs"].source_diagnostics["mode"] == "heuristic_bfs"


def test_diagnostics_and_reprs_are_text_safe(scattered_fixture: ScatteredFixture) -> None:
    """Diagnostics expose IDs/counts/statuses/scores but never text, vectors, or traces."""
    from research_graph.infrastructure.retrieval.hybrid import (
        InMemoryVectorCandidateIndex,  # noqa: PLC0415
    )
    from research_graph.workflows.rlm.graph_traversal import (  # noqa: PLC0415 - planned S10 API
        RLMGraphTraversalConfig,
        RLMGraphTraversalQuestion,
        compare_rlm_graph_traversal,
    )

    result = compare_rlm_graph_traversal(
        scattered_fixture.conn,
        RLMGraphTraversalQuestion(
            name="text-safety",
            query=scattered_fixture.query,
            query_vector=scattered_fixture.query_vector,
            seed_semantic_chunk_ids=scattered_fixture.seed_semantic_chunk_ids,
            seed_evidence_path_ids=scattered_fixture.seed_evidence_path_ids,
            expected_semantic_chunk_ids=scattered_fixture.expected_semantic_chunk_ids,
            expected_evidence_path_ids=scattered_fixture.expected_evidence_path_ids,
            optimizer_hint=RAW_OPTIMIZER_TEXT,
        ),
        vector_index=InMemoryVectorCandidateIndex.from_fixture_vectors(scattered_fixture.vectors),
        config=RLMGraphTraversalConfig(max_steps=4, max_neighbors_per_step=3, top_k=4),
    )

    safe_surfaces = {
        "result_repr": repr(result),
        "traversal_repr": repr(result.rlm_traversal),
        "trajectory_repr": repr(result.rlm_traversal.trajectory),
        "diagnostics_repr": repr(result.diagnostics),
        "baseline_repr": repr(result.baselines),
        "source_diagnostics_repr": repr(
            [baseline.source_diagnostics for baseline in result.baselines]
        ),
    }
    forbidden_fragments = [
        RAW_FIXTURE_BODY_TEXT,
        RAW_FIXTURE_CLAIM_TEXT,
        RAW_OPTIMIZER_TEXT,
        RAW_VECTOR_PAYLOAD,
        "deterministic local markdown before any network",
        "Local markdown is enough",
        "MIPRO optimizer trace",
        "query_vector",
        "values=(0.82",
        "embedding",
    ]

    leaks = [
        f"{surface}:{fragment}"
        for surface, rendered in safe_surfaces.items()
        for fragment in forbidden_fragments
        if fragment in rendered
    ]
    assert leaks == []
    assert set(result.diagnostics) >= {
        "question_id",
        "policy_label",
        "visited_count",
        "returned_count",
        "baseline_labels",
        "stop_reason",
        "budget_exhausted",
        "missing_expected_result_ids",
        "missing_expected_evidence_path_ids",
    }


def test_empty_seeds_are_rejected_before_traversal(scattered_fixture: ScatteredFixture) -> None:
    """Traversal without seed chunks/evidence should fail closed with an ID-only error."""
    from research_graph.infrastructure.retrieval.hybrid import (
        InMemoryVectorCandidateIndex,  # noqa: PLC0415
    )
    from research_graph.workflows.rlm.graph_traversal import (  # noqa: PLC0415 - planned S10 API
        RLMGraphTraversalConfig,
        RLMGraphTraversalQuestion,
        compare_rlm_graph_traversal,
    )

    with pytest.raises(ValueError, match="empty_seed") as exc_info:
        compare_rlm_graph_traversal(
            scattered_fixture.conn,
            RLMGraphTraversalQuestion(
                name="empty-seeds",
                query=scattered_fixture.query,
                query_vector=scattered_fixture.query_vector,
                seed_semantic_chunk_ids=(),
                seed_evidence_path_ids=(),
                expected_semantic_chunk_ids=scattered_fixture.expected_semantic_chunk_ids,
                expected_evidence_path_ids=scattered_fixture.expected_evidence_path_ids,
            ),
            vector_index=InMemoryVectorCandidateIndex.from_fixture_vectors(
                scattered_fixture.vectors
            ),
            config=RLMGraphTraversalConfig(max_steps=4, max_neighbors_per_step=3, top_k=4),
        )

    assert RAW_FIXTURE_BODY_TEXT not in str(exc_info.value)
    assert RAW_FIXTURE_CLAIM_TEXT not in str(exc_info.value)


def test_zero_traversal_budget_returns_budget_exhaustion_diagnostic(
    scattered_fixture: ScatteredFixture,
) -> None:
    """A zero step budget should not silently masquerade as successful traversal."""
    from research_graph.infrastructure.retrieval.hybrid import (
        InMemoryVectorCandidateIndex,  # noqa: PLC0415
    )
    from research_graph.workflows.rlm.graph_traversal import (  # noqa: PLC0415 - planned S10 API
        RLMGraphTraversalConfig,
        RLMGraphTraversalQuestion,
        compare_rlm_graph_traversal,
    )

    result = compare_rlm_graph_traversal(
        scattered_fixture.conn,
        RLMGraphTraversalQuestion(
            name="zero-budget",
            query=scattered_fixture.query,
            query_vector=scattered_fixture.query_vector,
            seed_semantic_chunk_ids=scattered_fixture.seed_semantic_chunk_ids,
            seed_evidence_path_ids=scattered_fixture.seed_evidence_path_ids,
            expected_semantic_chunk_ids=scattered_fixture.expected_semantic_chunk_ids,
            expected_evidence_path_ids=scattered_fixture.expected_evidence_path_ids,
        ),
        vector_index=InMemoryVectorCandidateIndex.from_fixture_vectors(scattered_fixture.vectors),
        config=RLMGraphTraversalConfig(max_steps=0, max_neighbors_per_step=3, top_k=4),
    )

    assert result.rlm_traversal.stop_reason == "budget_exhausted"
    assert result.rlm_traversal.budget_exhausted is True
    assert result.rlm_traversal.visited_semantic_chunk_ids == list(
        scattered_fixture.seed_semantic_chunk_ids
    )
    assert result.rlm_traversal.metrics.retrieval_recall < 1.0
    assert sorted(result.rlm_traversal.metrics.missing_expected_result_ids) == sorted(
        set(scattered_fixture.expected_semantic_chunk_ids)
        - set(scattered_fixture.seed_semantic_chunk_ids)
    )


def test_missing_expected_evidence_path_ids_are_reported_text_safely(
    scattered_fixture: ScatteredFixture,
) -> None:
    """Missing expected EvidencePath IDs should remain explicit metric diagnostics."""
    from research_graph.infrastructure.retrieval.hybrid import (
        InMemoryVectorCandidateIndex,  # noqa: PLC0415
    )
    from research_graph.workflows.rlm.graph_traversal import (  # noqa: PLC0415 - planned S10 API
        RLMGraphTraversalConfig,
        RLMGraphTraversalQuestion,
        compare_rlm_graph_traversal,
    )

    result = compare_rlm_graph_traversal(
        scattered_fixture.conn,
        RLMGraphTraversalQuestion(
            name="missing-expected-evidence",
            query=scattered_fixture.query,
            query_vector=scattered_fixture.query_vector,
            seed_semantic_chunk_ids=scattered_fixture.seed_semantic_chunk_ids,
            seed_evidence_path_ids=scattered_fixture.seed_evidence_path_ids,
            expected_semantic_chunk_ids=scattered_fixture.expected_semantic_chunk_ids,
            expected_evidence_path_ids={
                *scattered_fixture.expected_evidence_path_ids,
                "evidence:2605.12345:missing:2605.12345:missing:chunk-0001",
            },
        ),
        vector_index=InMemoryVectorCandidateIndex.from_fixture_vectors(scattered_fixture.vectors),
        config=RLMGraphTraversalConfig(max_steps=4, max_neighbors_per_step=3, top_k=4),
    )

    assert result.rlm_traversal.metrics.evidence_path_hit_rate < 1.0
    assert result.rlm_traversal.metrics.missing_expected_evidence_path_ids == [
        "evidence:2605.12345:missing:2605.12345:missing:chunk-0001"
    ]
    assert "Local markdown is enough" not in repr(result.rlm_traversal.metrics)


def test_duplicate_candidate_returns_are_deduplicated_and_diagnosed(
    scattered_fixture: ScatteredFixture,
) -> None:
    """Duplicate vector candidates must not inflate recall or evidence hit rate."""
    from research_graph.infrastructure.retrieval.hybrid import (
        InMemoryVectorCandidateIndex,  # noqa: PLC0415
    )
    from research_graph.workflows.rlm.graph_traversal import (  # noqa: PLC0415 - planned S10 API
        RLMGraphTraversalConfig,
        RLMGraphTraversalQuestion,
        compare_rlm_graph_traversal,
    )

    duplicate_vectors = [
        *scattered_fixture.vectors,
        FixtureVector(scattered_fixture.seed_semantic_chunk_ids[0], (1.0, 0.0, 0.0, 0.0)),
    ]
    result = compare_rlm_graph_traversal(
        scattered_fixture.conn,
        RLMGraphTraversalQuestion(
            name="duplicate-candidates",
            query=scattered_fixture.query,
            query_vector=scattered_fixture.query_vector,
            seed_semantic_chunk_ids=scattered_fixture.seed_semantic_chunk_ids,
            seed_evidence_path_ids=scattered_fixture.seed_evidence_path_ids,
            expected_semantic_chunk_ids=scattered_fixture.expected_semantic_chunk_ids,
            expected_evidence_path_ids=scattered_fixture.expected_evidence_path_ids,
        ),
        vector_index=InMemoryVectorCandidateIndex.from_fixture_vectors(duplicate_vectors),
        config=RLMGraphTraversalConfig(max_steps=4, max_neighbors_per_step=3, top_k=4),
    )

    assert result.rlm_traversal.returned_semantic_chunk_ids == sorted(
        set(result.rlm_traversal.returned_semantic_chunk_ids)
    )
    assert result.rlm_traversal.returned_evidence_path_ids == sorted(
        set(result.rlm_traversal.returned_evidence_path_ids)
    )
    assert result.rlm_traversal.diagnostics["duplicate_candidate_ids"] == [
        scattered_fixture.seed_semantic_chunk_ids[0]
    ]


def test_no_neighborhood_graph_traversal_returns_empty_neighborhood_status(
    scattered_fixture: ScatteredFixture,
) -> None:
    """Seeds with no graph neighborhood should return a deterministic stop reason."""
    from research_graph.infrastructure.retrieval.hybrid import (
        InMemoryVectorCandidateIndex,  # noqa: PLC0415
    )
    from research_graph.workflows.rlm.graph_traversal import (  # noqa: PLC0415 - planned S10 API
        RLMGraphTraversalConfig,
        RLMGraphTraversalQuestion,
        compare_rlm_graph_traversal,
    )

    result = compare_rlm_graph_traversal(
        scattered_fixture.conn,
        RLMGraphTraversalQuestion(
            name="no-neighborhood",
            query="no graph match",
            query_vector=(0.0, 0.0, 1.0, 0.0),
            seed_semantic_chunk_ids=("2605.12345:abstract:chunk-0001",),
            seed_evidence_path_ids=(),
            expected_semantic_chunk_ids=scattered_fixture.expected_semantic_chunk_ids,
            expected_evidence_path_ids=scattered_fixture.expected_evidence_path_ids,
        ),
        vector_index=InMemoryVectorCandidateIndex.from_fixture_vectors([]),
        config=RLMGraphTraversalConfig(max_steps=4, max_neighbors_per_step=3, top_k=4),
    )

    assert result.rlm_traversal.stop_reason == "empty_neighborhood"
    assert result.rlm_traversal.budget_exhausted is False
    assert result.rlm_traversal.returned_semantic_chunk_ids == ["2605.12345:abstract:chunk-0001"]
    assert result.rlm_traversal.metrics.retrieval_recall == pytest.approx(0.0)
    assert result.rlm_traversal.metrics.evidence_path_hit_rate == pytest.approx(0.0)
    assert result.rlm_traversal.diagnostics["empty_neighborhood"] is True
    assert result.rlm_traversal.diagnostics["missing_expected_result_ids"] == sorted(
        scattered_fixture.expected_semantic_chunk_ids
    )


def test_comparison_is_read_only_and_makes_no_adoption_recommendation(
    scattered_fixture: ScatteredFixture,
) -> None:
    """The comparison facade reports fixture metrics only and does not mutate inputs."""
    from research_graph.infrastructure.retrieval.hybrid import (
        InMemoryVectorCandidateIndex,  # noqa: PLC0415
    )
    from research_graph.workflows.rlm.graph_traversal import (  # noqa: PLC0415 - planned S10 API
        RLMGraphTraversalConfig,
        RLMGraphTraversalQuestion,
        compare_rlm_graph_traversal,
    )

    class RecordingReadOnlyConn:
        def __init__(self, delegate: ladybug.Connection) -> None:
            self.delegate = delegate
            self.queries: list[str] = []

        def execute(self, query: str, params: dict[str, Any] | None = None) -> Any:
            del params
            normalized = query.strip().upper()
            assert normalized.startswith("MATCH")
            assert not normalized.startswith(("BEGIN", "COMMIT", "ROLLBACK", "CREATE", "MERGE"))
            assert " SET " not in f" {normalized} "
            self.queries.append(query)
            return self.delegate.execute(query)

    question = RLMGraphTraversalQuestion(
        name="read-only-metrics-only",
        query=scattered_fixture.query,
        query_vector=scattered_fixture.query_vector,
        seed_semantic_chunk_ids=scattered_fixture.seed_semantic_chunk_ids,
        seed_evidence_path_ids=scattered_fixture.seed_evidence_path_ids,
        expected_semantic_chunk_ids=scattered_fixture.expected_semantic_chunk_ids,
        expected_evidence_path_ids=scattered_fixture.expected_evidence_path_ids,
    )
    vector_snapshot = deepcopy(scattered_fixture.vectors)
    question_snapshot = deepcopy(question.__dict__)
    conn = RecordingReadOnlyConn(scattered_fixture.conn)

    result = compare_rlm_graph_traversal(
        cast(ladybug.Connection, conn),
        question,
        vector_index=InMemoryVectorCandidateIndex.from_fixture_vectors(scattered_fixture.vectors),
        config=RLMGraphTraversalConfig(max_steps=4, max_neighbors_per_step=3, top_k=4),
    )

    assert conn.queries
    assert scattered_fixture.vectors == vector_snapshot
    assert question.__dict__ == question_snapshot
    rendered = repr(result).casefold()
    assert "recommend" not in rendered
    assert "adopt" not in rendered
    assert "production_quality" not in rendered
    assert "production-quality" not in rendered


def test_invalid_query_vector_and_empty_candidate_index_are_typed_diagnostics(
    scattered_fixture: ScatteredFixture,
) -> None:
    """Invalid vector dimensions should not hide graph/error diagnostics."""
    from research_graph.infrastructure.retrieval.hybrid import (
        InMemoryVectorCandidateIndex,  # noqa: PLC0415
    )
    from research_graph.workflows.rlm.graph_traversal import (  # noqa: PLC0415 - planned S10 API
        RLMGraphTraversalConfig,
        RLMGraphTraversalQuestion,
        compare_rlm_graph_traversal,
    )

    result = compare_rlm_graph_traversal(
        scattered_fixture.conn,
        RLMGraphTraversalQuestion(
            name="invalid-query-vector",
            query="no graph match",
            query_vector=(1.0, 0.0, 0.0, 0.0),
            seed_semantic_chunk_ids=scattered_fixture.seed_semantic_chunk_ids,
            seed_evidence_path_ids=scattered_fixture.seed_evidence_path_ids,
            expected_semantic_chunk_ids=scattered_fixture.expected_semantic_chunk_ids,
            expected_evidence_path_ids=scattered_fixture.expected_evidence_path_ids,
        ),
        vector_index=InMemoryVectorCandidateIndex.from_fixture_vectors(
            [FixtureVector("2605.12345:abstract:chunk-0001", (1.0, 0.0, 0.0))]
        ),
        config=RLMGraphTraversalConfig(max_steps=4, max_neighbors_per_step=3, top_k=4),
    )

    assert result.rlm_traversal.diagnostics["status"] == "error"
    assert result.rlm_traversal.diagnostics["error_type"] == "invalid_query_vector"
    by_label = {baseline.label: baseline for baseline in result.baselines}
    assert by_label["vector_only"].source_diagnostics["error_type"] == "invalid_query_vector"
    assert by_label["vector_only"].source_diagnostics["empty_candidate_index"] is True
    assert by_label["hybrid"].source_diagnostics["error_type"] == "invalid_query_vector"
    assert by_label["graph_one_hop"].source_diagnostics["candidate_count"] == 0
    assert by_label["heuristic_bfs"].source_diagnostics["mode"] == "heuristic_bfs"


def _rlm_graph_traversal_static_scope(path: Path) -> tuple[list[str], list[str]]:
    assert path.exists(), f"planned module missing: {path}"
    source = path.read_text(encoding="utf-8")
    tree = ast.parse(source, filename=str(path))
    imported_roots: list[str] = []
    runtime_refs: list[str] = []

    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            imported_roots.extend(alias.name.split(".")[0] for alias in node.names)
        elif isinstance(node, ast.ImportFrom) and node.module is not None:
            imported_roots.append(node.module.split(".")[0])
        elif isinstance(node, ast.Name):
            runtime_refs.append(node.id)
        elif isinstance(node, ast.Attribute):
            runtime_refs.append(node.attr)
        elif isinstance(node, ast.Constant) and isinstance(node.value, str):
            runtime_refs.append(node.value.upper())

    return imported_roots, runtime_refs


def test_rlm_graph_traversal_module_static_scope_is_read_only_and_optimizer_free() -> None:
    """The planned module must remain local, read-only, and optimizer-free."""
    imports, refs = _rlm_graph_traversal_static_scope(RLM_GRAPH_TRAVERSAL_MODULE)
    violations: list[str] = []
    bad_imports = sorted(set(imports) & FORBIDDEN_IMPORT_ROOTS)
    bad_refs = sorted(set(refs) & FORBIDDEN_RUNTIME_REFERENCES)

    if bad_imports:
        violations.append(f"{RLM_GRAPH_TRAVERSAL_MODULE} forbidden imports: {bad_imports}")
    if bad_refs:
        violations.append(f"{RLM_GRAPH_TRAVERSAL_MODULE} forbidden runtime refs: {bad_refs}")

    assert violations == []
