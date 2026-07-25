"""Contract tests for S07 fixture-backed evaluation benchmarks.

These tests define the public evaluation boundary over the deterministic S03
EvidencePath fixtures, S04 extraction patch contracts, and S06 retrieval result
shapes. They intentionally do not call DSPy, RLM traversal, optimizers, live
embeddings, or external services, and diagnostics must stay text-safe: IDs,
counts, modes, and query strings only.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
from typing import Any

import ladybug
import pytest

import research_graph.infrastructure.graph.ladybug_client as ladybug_client
from research_graph.infrastructure.graph.ladybug_client import evidence_path_id

# pyrefly: ignore [missing-import]
from tests.test_ladybug_scientific_kg import build_fixture_payload


def _as_mapping(value: Any) -> dict[str, Any]:
    """Normalize future dataclass or mapping results without coupling to internals."""
    if isinstance(value, dict):
        return value
    return value.__dict__


def _single_fixture_contract() -> tuple[Any, list[str], list[str], list[str], set[str]]:
    _document, _chunks, evidence_paths, patch = build_fixture_payload()
    expected_claim_ids = [patch.claims[0].claim_id]
    expected_entity_ids = [patch.entities[0].entity_id]
    expected_relation_ids = [patch.relations[0].relation_id]
    expected_evidence_ids = {evidence_path_id(path) for path in evidence_paths}
    return (
        patch,
        expected_claim_ids,
        expected_entity_ids,
        expected_relation_ids,
        expected_evidence_ids,
    )


@dataclass(frozen=True)
class FixtureVector:
    """Deterministic benchmark vector attached to one SemanticChunk."""

    semantic_chunk_id: str
    values: tuple[float, ...]


def test_extraction_benchmark_fixture_schema_validity_is_clean() -> None:
    """A known-good S04 fixture patch should be a valid benchmark input."""
    from research_graph.infrastructure.evaluation.evaluation_metrics import (  # noqa: PLC0415 - future public API contract
        ExtractionBenchmarkFixture,
        evaluate_schema_validity,
    )

    patch, expected_claim_ids, expected_entity_ids, expected_relation_ids, expected_evidence_ids = (
        _single_fixture_contract()
    )
    fixture = ExtractionBenchmarkFixture(
        name="local-markdown-pageindex",
        patch=patch,
        expected_claim_ids=expected_claim_ids,
        expected_entity_ids=expected_entity_ids,
        expected_relation_ids=expected_relation_ids,
        expected_evidence_path_ids=expected_evidence_ids,
    )

    result = evaluate_schema_validity(fixture)
    result_map = _as_mapping(result)

    assert result_map["valid"] is True
    assert result_map["diagnostics"] == []
    assert result_map["diagnostic_count"] == 0
    assert result_map["claim_count"] == 1
    assert result_map["entity_count"] == 1
    assert result_map["relation_count"] == 1


def test_groundedness_proxy_reports_expected_evidence_ids() -> None:
    """Groundedness checks count draft objects and compare derived evidence IDs."""
    from research_graph.infrastructure.evaluation.evaluation_metrics import (  # noqa: PLC0415 - future public API contract
        ExtractionBenchmarkFixture,
        evaluate_groundedness_proxy,
    )

    patch, expected_claim_ids, expected_entity_ids, expected_relation_ids, expected_evidence_ids = (
        _single_fixture_contract()
    )
    fixture = ExtractionBenchmarkFixture(
        name="local-markdown-pageindex",
        patch=patch,
        expected_claim_ids=expected_claim_ids,
        expected_entity_ids=expected_entity_ids,
        expected_relation_ids=expected_relation_ids,
        expected_evidence_path_ids=expected_evidence_ids,
    )

    result = evaluate_groundedness_proxy(fixture)
    result_map = _as_mapping(result)

    assert result_map["claim_count"] == 1
    assert result_map["entity_count"] == 1
    assert result_map["relation_count"] == 1
    assert result_map["evidence_backed_claim_count"] == 1
    assert result_map["evidence_backed_entity_count"] == 1
    assert result_map["evidence_backed_relation_count"] == 1
    assert result_map["derived_evidence_path_ids"] == sorted(expected_evidence_ids)
    assert result_map["missing_expected_evidence_path_ids"] == []
    assert result_map["unexpected_evidence_path_ids"] == []
    assert result_map["missing_evidence_path_draft_ids"] == []


def test_groundedness_proxy_names_missing_unexpected_and_none_evidence_ids() -> None:
    """Diagnostics should identify ID mismatches without exposing claim or chunk text."""
    from research_graph.infrastructure.evaluation.evaluation_metrics import (  # noqa: PLC0415 - future public API contract
        ExtractionBenchmarkFixture,
        evaluate_groundedness_proxy,
    )

    patch, expected_claim_ids, expected_entity_ids, expected_relation_ids, expected_evidence_ids = (
        _single_fixture_contract()
    )
    patch_with_missing_evidence = replace(
        patch,
        claims=[replace(patch.claims[0], evidence_path=None)],
    )
    fixture = ExtractionBenchmarkFixture(
        name="local-markdown-pageindex-broken-evidence",
        patch=patch_with_missing_evidence,
        expected_claim_ids=expected_claim_ids,
        expected_entity_ids=expected_entity_ids,
        expected_relation_ids=expected_relation_ids,
        expected_evidence_path_ids={*expected_evidence_ids, "evidence:missing:expected"},
    )

    result = evaluate_groundedness_proxy(fixture)
    result_map = _as_mapping(result)

    assert result_map["evidence_backed_claim_count"] == 0
    assert result_map["evidence_backed_entity_count"] == 1
    assert result_map["evidence_backed_relation_count"] == 1
    assert result_map["missing_evidence_path_draft_ids"] == [patch.claims[0].claim_id]
    assert result_map["missing_expected_evidence_path_ids"] == ["evidence:missing:expected"]
    assert result_map["unexpected_evidence_path_ids"] == []
    assert "Local markdown is enough" not in repr(result_map)


def test_evidence_path_hit_rate_handles_hits_misses_duplicates_and_none_ids() -> None:
    """Evidence hit rate should use unique non-null IDs and expose safe diagnostics."""
    from research_graph.infrastructure.evaluation.evaluation_metrics import (
        calculate_evidence_path_hit_rate,  # noqa: PLC0415
    )

    result = calculate_evidence_path_hit_rate(
        [
            {"retrieval_mode": "hybrid", "evidence_path_id": "evidence:a"},
            {"retrieval_mode": "hybrid", "evidence_path_id": "evidence:a"},
            {"retrieval_mode": "hybrid", "evidence_path_id": "evidence:x"},
            {"retrieval_mode": "hybrid", "evidence_path_id": None},
            {"retrieval_mode": "hybrid"},
        ],
        expected_evidence_path_ids={"evidence:a", "evidence:b"},
    )
    result_map = _as_mapping(result)

    assert result_map["hit_rate"] == pytest.approx(0.5)
    assert result_map["result_count"] == 5
    assert result_map["none_evidence_path_count"] == 2
    assert result_map["returned_evidence_path_ids"] == ["evidence:a", "evidence:x"]
    assert result_map["hit_evidence_path_ids"] == ["evidence:a"]
    assert result_map["missing_expected_evidence_path_ids"] == ["evidence:b"]
    assert result_map["unexpected_evidence_path_ids"] == ["evidence:x"]
    assert result_map["duplicate_evidence_path_ids"] == ["evidence:a"]


def test_evidence_path_hit_rate_empty_expected_sets_and_empty_results() -> None:
    """Empty expectations are vacuously satisfied only when no IDs are returned."""
    from research_graph.infrastructure.evaluation.evaluation_metrics import (
        calculate_evidence_path_hit_rate,  # noqa: PLC0415
    )

    empty_result = _as_mapping(
        calculate_evidence_path_hit_rate([], expected_evidence_path_ids=set())
    )
    unexpected_result = _as_mapping(
        calculate_evidence_path_hit_rate(
            [{"evidence_path_id": "evidence:unexpected"}],
            expected_evidence_path_ids=set(),
        )
    )

    assert empty_result["hit_rate"] == pytest.approx(1.0)
    assert empty_result["returned_evidence_path_ids"] == []
    assert empty_result["missing_expected_evidence_path_ids"] == []
    assert empty_result["unexpected_evidence_path_ids"] == []
    assert unexpected_result["hit_rate"] == pytest.approx(0.0)
    assert unexpected_result["unexpected_evidence_path_ids"] == ["evidence:unexpected"]


def test_retrieval_recall_handles_duplicates_missing_ids_none_ids_and_empty_lists() -> None:
    """Retrieval recall is based on unique result IDs and missing expected IDs."""
    from research_graph.infrastructure.evaluation.evaluation_metrics import (
        calculate_retrieval_recall,  # noqa: PLC0415
    )

    result = calculate_retrieval_recall(
        [
            {"semantic_chunk_id": "chunk:a", "evidence_path_id": "evidence:a"},
            {"semantic_chunk_id": "chunk:a", "evidence_path_id": "evidence:a"},
            {"semantic_chunk_id": "chunk:x", "evidence_path_id": "evidence:x"},
            {"semantic_chunk_id": None, "evidence_path_id": None},
            {"evidence_path_id": "evidence:y"},
        ],
        expected_result_ids={"chunk:a", "chunk:b"},
        result_id_field="semantic_chunk_id",
    )
    result_map = _as_mapping(result)

    assert result_map["recall"] == pytest.approx(0.5)
    assert result_map["result_count"] == 5
    assert result_map["none_result_id_count"] == 2
    assert result_map["returned_result_ids"] == ["chunk:a", "chunk:x"]
    assert result_map["matched_expected_result_ids"] == ["chunk:a"]
    assert result_map["missing_expected_result_ids"] == ["chunk:b"]
    assert result_map["unexpected_result_ids"] == ["chunk:x"]
    assert result_map["duplicate_result_ids"] == ["chunk:a"]

    empty_expected = _as_mapping(
        calculate_retrieval_recall(
            [], expected_result_ids=set(), result_id_field="semantic_chunk_id"
        )
    )
    empty_results_with_expected = _as_mapping(
        calculate_retrieval_recall(
            [],
            expected_result_ids={"chunk:missing"},
            result_id_field="semantic_chunk_id",
        )
    )

    assert empty_expected["recall"] == pytest.approx(1.0)
    assert empty_expected["missing_expected_result_ids"] == []
    assert empty_results_with_expected["recall"] == pytest.approx(0.0)
    assert empty_results_with_expected["missing_expected_result_ids"] == ["chunk:missing"]


def test_retrieval_ablation_runner_exercises_s05_fixture_and_s06_modes() -> None:
    """Ablation benchmarks should compose S05 persistence with S06 retrieval modes."""
    from research_graph.infrastructure.evaluation.evaluation_metrics import (  # noqa: PLC0415 - public benchmark contract
        BenchmarkRetrievalQuestion,
        run_retrieval_ablations,
    )
    from research_graph.infrastructure.retrieval.hybrid import (  # noqa: PLC0415 - public retrieval contract
        HybridRetrievalMode,
        InMemoryVectorCandidateIndex,
    )

    db = ladybug.Database(":memory:")
    conn = ladybug.Connection(db)
    ladybug_client.init_scientific_kg_schema(conn)
    document, chunks, evidence_paths, patch = build_fixture_payload()
    ladybug_client.upsert_scientific_kg(conn, document, chunks, evidence_paths, patch)

    expected_chunk_id = "2605.12345:method:chunk-0001"
    expected_evidence_id = "evidence:2605.12345:method:2605.12345:method:chunk-0001"
    assert {evidence_path_id(path) for path in evidence_paths} == {expected_evidence_id}

    results = run_retrieval_ablations(
        conn,
        [
            BenchmarkRetrievalQuestion(
                name="method-pageindex-traceability",
                query="PageIndex",
                query_vector=(1.0, 0.0, 0.0),
                expected_result_ids={expected_chunk_id},
                expected_evidence_path_ids={expected_evidence_id},
            )
        ],
        InMemoryVectorCandidateIndex.from_fixture_vectors(
            [
                FixtureVector(expected_chunk_id, (1.0, 0.0, 0.0)),
                FixtureVector("2605.12345:abstract:chunk-0001", (0.0, 1.0, 0.0)),
            ]
        ),
        modes=(
            HybridRetrievalMode.VECTOR_ONLY,
            HybridRetrievalMode.GRAPH_ONLY,
            HybridRetrievalMode.HYBRID,
        ),
        top_k=3,
    )

    by_mode = {result.mode: result for result in results}
    assert set(by_mode) == {
        HybridRetrievalMode.VECTOR_ONLY,
        HybridRetrievalMode.GRAPH_ONLY,
        HybridRetrievalMode.HYBRID,
    }
    for mode, result in by_mode.items():
        assert result.question_id == "method-pageindex-traceability"
        assert result.mode is mode
        assert result.top_k == 3
        assert result.returned_semantic_chunk_ids == [expected_chunk_id]
        assert result.returned_evidence_path_ids == [expected_evidence_id]
        assert result.evidence_path_hit_rate == pytest.approx(1.0)
        assert result.retrieval_recall == pytest.approx(1.0)
        assert result.missing_expected_evidence_path_ids == []
        assert result.missing_expected_result_ids == []
        assert result.s06_diagnostics["query_text"] == "PageIndex"
        assert "Local markdown is enough to build a deterministic PageIndex." not in repr(result)

    assert by_mode[HybridRetrievalMode.VECTOR_ONLY].s06_diagnostics["graph_candidate_count"] is None
    assert by_mode[HybridRetrievalMode.GRAPH_ONLY].s06_diagnostics["vector_candidate_count"] == 0
    assert by_mode[HybridRetrievalMode.HYBRID].s06_diagnostics["graph_evidence_path_ids"] == [
        expected_evidence_id
    ]


def test_retrieval_ablation_runner_reports_empty_results_and_missing_ids() -> None:
    """Empty vector/graph neighborhoods should score zero without false success."""
    from research_graph.infrastructure.evaluation.evaluation_metrics import (  # noqa: PLC0415 - public benchmark contract
        BenchmarkRetrievalQuestion,
        run_retrieval_ablations,
    )
    from research_graph.infrastructure.retrieval.hybrid import (  # noqa: PLC0415 - public retrieval contract
        HybridRetrievalMode,
        InMemoryVectorCandidateIndex,
    )

    db = ladybug.Database(":memory:")
    conn = ladybug.Connection(db)
    ladybug_client.init_scientific_kg_schema(conn)
    document, chunks, evidence_paths, patch = build_fixture_payload()
    ladybug_client.upsert_scientific_kg(conn, document, chunks, evidence_paths, patch)

    result = run_retrieval_ablations(
        conn,
        [
            BenchmarkRetrievalQuestion(
                name="missing-fixture-traceability",
                query="no graph match",
                query_vector=(1.0, 0.0, 0.0),
                expected_result_ids={"chunk:missing"},
                expected_evidence_path_ids={"evidence:missing"},
            )
        ],
        InMemoryVectorCandidateIndex.from_fixture_vectors([]),
        modes=(HybridRetrievalMode.HYBRID,),
        top_k=3,
    )[0]

    assert result.returned_semantic_chunk_ids == []
    assert result.returned_evidence_path_ids == []
    assert result.evidence_path_hit_rate == pytest.approx(0.0)
    assert result.retrieval_recall == pytest.approx(0.0)
    assert result.missing_expected_evidence_path_ids == ["evidence:missing"]
    assert result.missing_expected_result_ids == ["chunk:missing"]
    assert result.s06_diagnostics["empty_vector_candidates"] is True
    assert result.s06_diagnostics["empty_graph_candidates"] is True
    assert result.s06_diagnostics["empty_graph_reason"] == "no_scientific_kg_matches"
    assert "Local markdown is enough to build a deterministic PageIndex." not in repr(result)
