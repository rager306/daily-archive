"""Contract tests for S06 fixture-level hybrid retrieval.

These tests define the retrieval boundary that combines deterministic in-test
vector similarity, read-only LadybugDB graph expansion, and stable fusion over
S05 scientific KG fixtures. They intentionally do not call embedding services,
log paper text, or require live credentials.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, cast

import ladybug
import pytest

import arxiv_archive.ladybug_client as ladybug_client
from tests.test_ladybug_scientific_kg import build_fixture_payload


@dataclass(frozen=True)
class FixtureVector:
    """Deterministic test-only vector attached to one SemanticChunk."""

    semantic_chunk_id: str
    values: tuple[float, ...]


@pytest.fixture()
def scientific_kg_conn() -> ladybug.Connection:
    db = ladybug.Database(":memory:")
    conn = ladybug.Connection(db)
    ladybug_client.init_scientific_kg_schema(conn)
    document, chunks, evidence_paths, patch = build_fixture_payload()
    ladybug_client.upsert_scientific_kg(conn, document, chunks, evidence_paths, patch)
    return conn


@pytest.fixture()
def fixture_vectors() -> list[FixtureVector]:
    return [
        FixtureVector("2605.12345:method:chunk-0001", (1.0, 0.0, 0.0)),
        FixtureVector("2605.12345:abstract:chunk-0001", (0.0, 1.0, 0.0)),
        FixtureVector("2605.12345:conclusion:chunk-0001", (0.0, 0.0, 1.0)),
    ]


def _result_rows(response: Any) -> list[dict[str, Any]]:
    """Normalize future dataclass or mapping responses for shape assertions."""
    raw_results = getattr(response, "results", response["results"])
    rows: list[dict[str, Any]] = []
    for result in raw_results:
        if isinstance(result, dict):
            rows.append(result)
        else:
            rows.append(result.__dict__)
    return rows


def _diagnostics(response: Any) -> dict[str, Any]:
    diagnostics = getattr(response, "diagnostics", response["diagnostics"])
    return diagnostics if isinstance(diagnostics, dict) else diagnostics.__dict__


def test_vector_only_retrieval_returns_evidence_backed_score_metadata(
    scientific_kg_conn: ladybug.Connection,
    fixture_vectors: list[FixtureVector],
) -> None:
    """Vector-only retrieval exposes stable metadata without graph scores."""
    from research_graph.retrieval.hybrid import (  # noqa: PLC0415 - future public API contract
        HybridRetrievalMode,
        HybridRetrievalQuery,
        InMemoryVectorCandidateIndex,
        retrieve_hybrid,
    )

    response = retrieve_hybrid(
        scientific_kg_conn,
        HybridRetrievalQuery(
            text="local markdown page index method",
            vector=(1.0, 0.0, 0.0),
            mode=HybridRetrievalMode.VECTOR_ONLY,
            limit=3,
        ),
        vector_index=InMemoryVectorCandidateIndex.from_fixture_vectors(fixture_vectors),
    )

    rows = _result_rows(response)
    assert rows
    assert rows[0] == {
        "retrieval_mode": "vector_only",
        "candidate_source": "vector",
        "semantic_chunk_id": "2605.12345:method:chunk-0001",
        "page_index_node_id": "2605.12345:method",
        "evidence_path_id": "evidence:2605.12345:method:2605.12345:method:chunk-0001",
        "vector_score": pytest.approx(1.0),
        "graph_score": None,
        "fusion_score": pytest.approx(1.0),
    }
    assert _diagnostics(response) == {
        "query_text": "local markdown page index method",
        "vector_candidate_count": 1,
        "graph_candidate_count": None,
        "empty_vector_candidates": False,
        "empty_graph_candidates": None,
        "empty_graph_reason": None,
        "graph_evidence_path_ids": [],
        "missing_evidence_path_links": [],
    }


def test_graph_only_retrieval_expands_from_scientific_kg_without_vectors(
    scientific_kg_conn: ladybug.Connection,
) -> None:
    """Graph-only retrieval finds evidence-linked claims/entities through LadybugDB."""
    from research_graph.retrieval.hybrid import (  # noqa: PLC0415 - future public API contract
        HybridRetrievalMode,
        HybridRetrievalQuery,
        retrieve_hybrid,
    )

    response = retrieve_hybrid(
        scientific_kg_conn,
        HybridRetrievalQuery(
            text="PageIndex",
            vector=None,
            mode=HybridRetrievalMode.GRAPH_ONLY,
            limit=3,
        ),
    )

    rows = _result_rows(response)
    assert rows == [
        {
            "retrieval_mode": "graph_only",
            "candidate_source": "graph",
            "semantic_chunk_id": "2605.12345:method:chunk-0001",
            "page_index_node_id": "2605.12345:method",
            "evidence_path_id": "evidence:2605.12345:method:2605.12345:method:chunk-0001",
            "vector_score": None,
            "graph_score": pytest.approx(0.91),
            "fusion_score": pytest.approx(0.91),
        }
    ]
    assert _diagnostics(response) == {
        "query_text": "PageIndex",
        "vector_candidate_count": 0,
        "graph_candidate_count": 1,
        "empty_vector_candidates": None,
        "empty_graph_candidates": False,
        "empty_graph_reason": None,
        "graph_evidence_path_ids": ["evidence:2605.12345:method:2605.12345:method:chunk-0001"],
        "missing_evidence_path_links": [],
    }


def test_graph_only_retrieval_expands_relation_neighborhood_from_endpoint_match(
    scientific_kg_conn: ladybug.Connection,
) -> None:
    """Graph expansion uses SCIENTIFIC_RELATION endpoint edges without exposing text."""
    from research_graph.retrieval.hybrid import (  # noqa: PLC0415 - future public API contract
        HybridRetrievalMode,
        HybridRetrievalQuery,
        retrieve_hybrid,
    )

    response = retrieve_hybrid(
        scientific_kg_conn,
        HybridRetrievalQuery(
            text="Local markdown is enough",
            vector=None,
            mode=HybridRetrievalMode.GRAPH_ONLY,
            limit=3,
        ),
    )

    rows = _result_rows(response)
    assert rows == [
        {
            "retrieval_mode": "graph_only",
            "candidate_source": "graph",
            "semantic_chunk_id": "2605.12345:method:chunk-0001",
            "page_index_node_id": "2605.12345:method",
            "evidence_path_id": "evidence:2605.12345:method:2605.12345:method:chunk-0001",
            "vector_score": None,
            "graph_score": pytest.approx(0.91),
            "fusion_score": pytest.approx(0.91),
        }
    ]
    diagnostics = _diagnostics(response)
    assert diagnostics["graph_candidate_count"] == 1
    assert diagnostics["graph_evidence_path_ids"] == [
        "evidence:2605.12345:method:2605.12345:method:chunk-0001"
    ]
    assert "Local markdown is enough" == diagnostics["query_text"]
    assert (
        "Local markdown is enough to build a deterministic PageIndex." not in diagnostics.values()
    )


def test_graph_retrieval_does_not_open_write_transactions_or_mutate_schema(
    scientific_kg_conn: ladybug.Connection,
) -> None:
    """Retrieval must be read-only: no BEGIN, CREATE, MERGE, SET, COMMIT, or ROLLBACK."""
    from research_graph.retrieval.hybrid import (  # noqa: PLC0415 - future public API contract
        HybridRetrievalMode,
        HybridRetrievalQuery,
        retrieve_hybrid,
    )

    class RecordingReadOnlyConn:
        def __init__(self, delegate: ladybug.Connection) -> None:
            self.delegate = delegate
            self.queries: list[str] = []

        def execute(self, query: str, params: dict[str, Any] | None = None) -> Any:
            del params
            normalized = query.strip().upper()
            assert not normalized.startswith(("BEGIN", "COMMIT", "ROLLBACK", "CREATE", "MERGE"))
            assert " SET " not in f" {normalized} "
            self.queries.append(query)
            return self.delegate.execute(query)

    conn = RecordingReadOnlyConn(scientific_kg_conn)
    response = retrieve_hybrid(
        cast(ladybug.Connection, conn),
        HybridRetrievalQuery(
            text="supports",
            vector=None,
            mode=HybridRetrievalMode.GRAPH_ONLY,
            limit=3,
        ),
    )

    assert _result_rows(response)
    assert conn.queries
    assert all(query.strip().upper().startswith("MATCH") for query in conn.queries)


def test_hybrid_retrieval_stably_fuses_vector_and_graph_candidates(
    scientific_kg_conn: ladybug.Connection,
    fixture_vectors: list[FixtureVector],
) -> None:
    """Hybrid retrieval de-duplicates candidates and keeps per-source scores."""
    from research_graph.retrieval.hybrid import (  # noqa: PLC0415 - future public API contract
        HybridRetrievalMode,
        HybridRetrievalQuery,
        InMemoryVectorCandidateIndex,
        retrieve_hybrid,
    )

    response = retrieve_hybrid(
        scientific_kg_conn,
        HybridRetrievalQuery(
            text="PageIndex",
            vector=(1.0, 0.0, 0.0),
            mode=HybridRetrievalMode.HYBRID,
            limit=3,
        ),
        vector_index=InMemoryVectorCandidateIndex.from_fixture_vectors(fixture_vectors),
        vector_weight=0.7,
        graph_weight=0.3,
    )

    rows = _result_rows(response)
    assert rows[0] == {
        "retrieval_mode": "hybrid",
        "candidate_source": "vector+graph",
        "semantic_chunk_id": "2605.12345:method:chunk-0001",
        "page_index_node_id": "2605.12345:method",
        "evidence_path_id": "evidence:2605.12345:method:2605.12345:method:chunk-0001",
        "vector_score": pytest.approx(1.0),
        "graph_score": pytest.approx(0.91),
        "fusion_score": pytest.approx(0.973),
    }
    assert _diagnostics(response) == {
        "query_text": "PageIndex",
        "vector_candidate_count": 1,
        "graph_candidate_count": 1,
        "empty_vector_candidates": False,
        "empty_graph_candidates": False,
        "empty_graph_reason": None,
        "graph_evidence_path_ids": ["evidence:2605.12345:method:2605.12345:method:chunk-0001"],
        "missing_evidence_path_links": [],
    }


def test_hybrid_retrieval_reports_empty_candidate_sets_and_missing_evidence_links(
    scientific_kg_conn: ladybug.Connection,
) -> None:
    """Diagnostics identify empty vector/graph sides and chunks lacking EvidencePath links."""
    from research_graph.retrieval.hybrid import (  # noqa: PLC0415 - future public API contract
        HybridRetrievalMode,
        HybridRetrievalQuery,
        InMemoryVectorCandidateIndex,
        retrieve_hybrid,
    )

    response = retrieve_hybrid(
        scientific_kg_conn,
        HybridRetrievalQuery(
            text="no graph match",
            vector=(0.0, 1.0, 0.0),
            mode=HybridRetrievalMode.HYBRID,
            limit=3,
        ),
        vector_index=InMemoryVectorCandidateIndex.from_fixture_vectors(
            [FixtureVector("2605.12345:abstract:chunk-0001", (0.0, 1.0, 0.0))]
        ),
    )

    rows = _result_rows(response)
    assert rows == [
        {
            "retrieval_mode": "hybrid",
            "candidate_source": "vector",
            "semantic_chunk_id": "2605.12345:abstract:chunk-0001",
            "page_index_node_id": "2605.12345:abstract",
            "evidence_path_id": None,
            "vector_score": pytest.approx(1.0),
            "graph_score": None,
            "fusion_score": pytest.approx(0.7),
        }
    ]
    assert _diagnostics(response) == {
        "query_text": "no graph match",
        "vector_candidate_count": 1,
        "graph_candidate_count": 0,
        "empty_vector_candidates": False,
        "empty_graph_candidates": True,
        "empty_graph_reason": "no_scientific_kg_matches",
        "graph_evidence_path_ids": [],
        "missing_evidence_path_links": ["2605.12345:abstract:chunk-0001"],
    }

    empty_response = retrieve_hybrid(
        scientific_kg_conn,
        HybridRetrievalQuery(
            text="still no graph match",
            vector=(1.0, 0.0, 0.0),
            mode=HybridRetrievalMode.HYBRID,
            limit=3,
        ),
        vector_index=InMemoryVectorCandidateIndex.from_fixture_vectors([]),
    )

    assert _result_rows(empty_response) == []
    assert _diagnostics(empty_response) == {
        "query_text": "still no graph match",
        "vector_candidate_count": 0,
        "graph_candidate_count": 0,
        "empty_vector_candidates": True,
        "empty_graph_candidates": True,
        "empty_graph_reason": "no_scientific_kg_matches",
        "graph_evidence_path_ids": [],
        "missing_evidence_path_links": [],
    }
