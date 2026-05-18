"""Fixture-level hybrid retrieval over vectors and the scientific KG.

This module intentionally stays deterministic and local-only: callers provide
query vectors directly, graph expansion uses read-only LadybugDB queries, and no
embedding service, paper text, credentials, or live network dependency is used.
"""

from __future__ import annotations

import math
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass
from enum import StrEnum
from typing import Any, cast

import ladybug


class HybridRetrievalMode(StrEnum):
    """Supported fixture retrieval modes."""

    VECTOR_ONLY = "vector_only"
    GRAPH_ONLY = "graph_only"
    HYBRID = "hybrid"


@dataclass(frozen=True)
class HybridRetrievalQuery:
    """Deterministic retrieval query payload."""

    text: str
    vector: tuple[float, ...] | None
    mode: HybridRetrievalMode
    limit: int = 10


@dataclass(frozen=True)
class VectorCandidate:
    """One scored vector-side candidate."""

    semantic_chunk_id: str
    vector_score: float


@dataclass(frozen=True)
class GraphCandidate:
    """One scored graph-side candidate backed by an EvidencePath."""

    semantic_chunk_id: str
    page_index_node_id: str
    evidence_path_id: str
    graph_score: float
    graph_source: str


@dataclass(frozen=True)
class GraphExpansion:
    """Read-only graph expansion result plus text-free diagnostics."""

    candidates: list[GraphCandidate]
    diagnostics: dict[str, Any]


@dataclass(frozen=True)
class HybridRetrievalResponse:
    """Hybrid retrieval response with structured diagnostics."""

    results: list[dict[str, Any]]
    diagnostics: dict[str, Any]

    def __getitem__(self, key: str) -> Any:
        """Allow mapping-style access for contract-test compatibility."""
        if key == "results":
            return self.results
        if key == "diagnostics":
            return self.diagnostics
        raise KeyError(key)


class InMemoryVectorCandidateIndex:
    """Deterministic in-memory vector index used by fixture tests."""

    def __init__(self, vectors: Mapping[str, tuple[float, ...]]) -> None:
        self._vectors = dict(vectors)

    @classmethod
    def from_fixture_vectors(cls, fixture_vectors: Iterable[Any]) -> InMemoryVectorCandidateIndex:
        """Build an index from test fixture objects with id/vector attributes."""
        vectors: dict[str, tuple[float, ...]] = {}
        for item in fixture_vectors:
            semantic_chunk_id = cast(str, item.semantic_chunk_id)
            values = tuple(float(value) for value in item.values)
            vectors[semantic_chunk_id] = values
        return cls(vectors)

    def search(self, query_vector: Sequence[float], *, limit: int) -> list[VectorCandidate]:
        """Return stable top-k cosine-similarity candidates."""
        scored = [
            VectorCandidate(semantic_chunk_id=semantic_chunk_id, vector_score=score)
            for semantic_chunk_id, values in self._vectors.items()
            if (score := _cosine_similarity(query_vector, values)) > 0.0
        ]
        scored.sort(key=lambda candidate: (-candidate.vector_score, candidate.semantic_chunk_id))
        return scored[: max(limit, 0)]


def retrieve_hybrid(
    conn: ladybug.Connection,
    query: HybridRetrievalQuery,
    *,
    vector_index: InMemoryVectorCandidateIndex | None = None,
    vector_weight: float = 0.7,
    graph_weight: float = 0.3,
) -> HybridRetrievalResponse:
    """Retrieve fixture candidates with vector scoring, graph expansion, or both."""
    use_vector = query.mode in {HybridRetrievalMode.VECTOR_ONLY, HybridRetrievalMode.HYBRID}
    use_graph = query.mode in {HybridRetrievalMode.GRAPH_ONLY, HybridRetrievalMode.HYBRID}

    vector_candidates: list[VectorCandidate] = []
    if use_vector and vector_index is not None and query.vector is not None:
        vector_candidates = vector_index.search(query.vector, limit=query.limit)

    graph_expansion = GraphExpansion(candidates=[], diagnostics=_empty_graph_diagnostics(query.text, reason=None))
    if use_graph:
        graph_expansion = _expand_graph_candidates(conn, query.text, limit=query.limit)

    evidence_by_chunk = _evidence_paths_by_chunk(conn)
    results = _fuse_results(
        mode=query.mode,
        vector_candidates=vector_candidates,
        graph_candidates=graph_expansion.candidates,
        evidence_by_chunk=evidence_by_chunk,
        limit=query.limit,
        vector_weight=vector_weight,
        graph_weight=graph_weight,
    )

    missing_evidence_path_links = [
        row["semantic_chunk_id"]
        for row in results
        if row["evidence_path_id"] is None
    ]

    diagnostics = {
        "query_text": query.text,
        "vector_candidate_count": len(vector_candidates),
        "graph_candidate_count": len(graph_expansion.candidates) if use_graph else None,
        "empty_vector_candidates": (not vector_candidates) if use_vector else None,
        "empty_graph_candidates": (not graph_expansion.candidates) if use_graph else None,
        "empty_graph_reason": graph_expansion.diagnostics["empty_graph_reason"] if use_graph else None,
        "graph_evidence_path_ids": graph_expansion.diagnostics["evidence_path_ids"] if use_graph else [],
        "missing_evidence_path_links": missing_evidence_path_links,
    }
    return HybridRetrievalResponse(results=results, diagnostics=diagnostics)


def _fuse_results(
    *,
    mode: HybridRetrievalMode,
    vector_candidates: list[VectorCandidate],
    graph_candidates: list[GraphCandidate],
    evidence_by_chunk: Mapping[str, tuple[str, str]],
    limit: int,
    vector_weight: float,
    graph_weight: float,
) -> list[dict[str, Any]]:
    vector_by_chunk = {candidate.semantic_chunk_id: candidate for candidate in vector_candidates}
    graph_by_chunk = {candidate.semantic_chunk_id: candidate for candidate in graph_candidates}
    semantic_chunk_ids = sorted(set(vector_by_chunk) | set(graph_by_chunk))

    rows: list[dict[str, Any]] = []
    for semantic_chunk_id in semantic_chunk_ids:
        vector_candidate = vector_by_chunk.get(semantic_chunk_id)
        graph_candidate = graph_by_chunk.get(semantic_chunk_id)
        vector_score = vector_candidate.vector_score if vector_candidate is not None else None
        graph_score = graph_candidate.graph_score if graph_candidate is not None else None
        evidence = evidence_by_chunk.get(semantic_chunk_id)
        page_index_node_id = (
            graph_candidate.page_index_node_id
            if graph_candidate is not None
            else evidence[1]
            if evidence is not None
            else _page_index_node_id_from_chunk(semantic_chunk_id)
        )
        evidence_path_id = graph_candidate.evidence_path_id if graph_candidate is not None else evidence[0] if evidence else None
        rows.append(
            {
                "retrieval_mode": mode.value,
                "candidate_source": _candidate_source(vector_candidate is not None, graph_candidate is not None),
                "semantic_chunk_id": semantic_chunk_id,
                "page_index_node_id": page_index_node_id,
                "evidence_path_id": evidence_path_id,
                "vector_score": vector_score,
                "graph_score": graph_score,
                "fusion_score": _fusion_score(
                    mode=mode,
                    vector_score=vector_score,
                    graph_score=graph_score,
                    vector_weight=vector_weight,
                    graph_weight=graph_weight,
                ),
            }
        )

    rows.sort(key=lambda row: (-cast(float, row["fusion_score"]), cast(str, row["semantic_chunk_id"])))
    return rows[: max(limit, 0)]


def _candidate_source(has_vector: bool, has_graph: bool) -> str:
    if has_vector and has_graph:
        return "vector+graph"
    if has_vector:
        return "vector"
    return "graph"


def _fusion_score(
    *,
    mode: HybridRetrievalMode,
    vector_score: float | None,
    graph_score: float | None,
    vector_weight: float,
    graph_weight: float,
) -> float:
    if mode is HybridRetrievalMode.VECTOR_ONLY:
        return float(vector_score or 0.0)
    if mode is HybridRetrievalMode.GRAPH_ONLY:
        return float(graph_score or 0.0)
    return float((vector_score or 0.0) * vector_weight + (graph_score or 0.0) * graph_weight)


def _cosine_similarity(left: Sequence[float], right: Sequence[float]) -> float:
    if len(left) != len(right):
        raise ValueError("query vector and candidate vector dimensions differ")
    left_norm = math.sqrt(sum(value * value for value in left))
    right_norm = math.sqrt(sum(value * value for value in right))
    if left_norm == 0.0 or right_norm == 0.0:
        return 0.0
    dot = sum(left_value * right_value for left_value, right_value in zip(left, right, strict=True))
    return dot / (left_norm * right_norm)


def _expand_graph_candidates(conn: ladybug.Connection, text: str, *, limit: int) -> GraphExpansion:
    """Expand SCI KG neighborhoods through read-only evidence-backed queries."""
    needle = text.casefold().strip()
    if not needle:
        return GraphExpansion(candidates=[], diagnostics=_empty_graph_diagnostics(text, reason="blank_query"))

    rows: dict[str, GraphCandidate] = {}
    for candidate in _direct_scientific_kg_candidates(conn, needle):
        _keep_best_graph_candidate(rows, candidate)
    for candidate in _relation_neighborhood_candidates(conn, needle):
        _keep_best_graph_candidate(rows, candidate)

    candidates = list(rows.values())
    candidates.sort(key=lambda candidate: (-candidate.graph_score, candidate.semantic_chunk_id, candidate.evidence_path_id))
    limited = candidates[: max(limit, 0)]
    reason = None if limited else "no_scientific_kg_matches"
    return GraphExpansion(
        candidates=limited,
        diagnostics={
            "query_text": text,
            "candidate_count": len(limited),
            "empty_graph_reason": reason,
            "evidence_path_ids": [candidate.evidence_path_id for candidate in limited],
        },
    )


# Backwards-compatible name for tests or callers that imported the T02 helper.
def _graph_candidates(conn: ladybug.Connection, text: str, *, limit: int) -> list[GraphCandidate]:
    return _expand_graph_candidates(conn, text, limit=limit).candidates


def _direct_scientific_kg_candidates(conn: ladybug.Connection, needle: str) -> list[GraphCandidate]:
    candidates: list[GraphCandidate] = []
    for label, text_property, source_name in [
        ("Claim", "text", "claim"),
        ("ScientificEntity", "label", "entity"),
        ("ScientificRelation", "relation_type", "relation"),
    ]:
        result = conn.execute(
            f"MATCH (item:{label})-[:EVIDENCED_BY]->(evidence:EvidencePath) "
            "RETURN item."
            f"{text_property}, item.confidence, evidence.id, evidence.page_index_node_id, evidence.semantic_chunk_id"
        )
        while result.has_next():
            item_text, confidence, evidence_id, page_index_node_id, semantic_chunk_id = result.get_next()
            if needle not in str(item_text).casefold():
                continue
            candidates.append(
                GraphCandidate(
                    semantic_chunk_id=str(semantic_chunk_id),
                    page_index_node_id=str(page_index_node_id),
                    evidence_path_id=str(evidence_id),
                    graph_score=_bounded_score(confidence),
                    graph_source=source_name,
                )
            )
    return candidates


def _relation_neighborhood_candidates(conn: ladybug.Connection, needle: str) -> list[GraphCandidate]:
    """Return one-hop relation endpoint evidence without mutating the graph."""
    candidates: list[GraphCandidate] = []
    for endpoint_label, text_property, endpoint_role in [
        ("Claim", "text", "claim_endpoint"),
        ("ScientificEntity", "label", "entity_endpoint"),
    ]:
        for rel_table in ["SCIENTIFIC_RELATION_SOURCE", "SCIENTIFIC_RELATION_TARGET"]:
            result = conn.execute(
                f"MATCH (relation:ScientificRelation)-[:{rel_table}]->(endpoint:{endpoint_label}), "
                "(relation)-[:EVIDENCED_BY]->(evidence:EvidencePath) "
                "RETURN endpoint."
                f"{text_property}, relation.confidence, evidence.id, evidence.page_index_node_id, evidence.semantic_chunk_id"
            )
            while result.has_next():
                endpoint_text, confidence, evidence_id, page_index_node_id, semantic_chunk_id = result.get_next()
                if needle not in str(endpoint_text).casefold():
                    continue
                candidates.append(
                    GraphCandidate(
                        semantic_chunk_id=str(semantic_chunk_id),
                        page_index_node_id=str(page_index_node_id),
                        evidence_path_id=str(evidence_id),
                        graph_score=_bounded_score(confidence) * 0.9,
                        graph_source=endpoint_role,
                    )
                )
    return candidates


def _keep_best_graph_candidate(rows: dict[str, GraphCandidate], candidate: GraphCandidate) -> None:
    current = rows.get(candidate.semantic_chunk_id)
    if current is None or (candidate.graph_score, candidate.evidence_path_id) > (
        current.graph_score,
        current.evidence_path_id,
    ):
        rows[candidate.semantic_chunk_id] = candidate


def _bounded_score(value: Any) -> float:
    try:
        score = float(value)
    except (TypeError, ValueError):
        return 1.0
    return min(max(score, 0.0), 1.0)


def _empty_graph_diagnostics(text: str, *, reason: str | None) -> dict[str, Any]:
    return {
        "query_text": text,
        "candidate_count": 0,
        "empty_graph_reason": reason,
        "evidence_path_ids": [],
    }


def _evidence_paths_by_chunk(conn: ladybug.Connection) -> dict[str, tuple[str, str]]:
    result = conn.execute(
        "MATCH (evidence:EvidencePath) "
        "RETURN evidence.semantic_chunk_id, evidence.id, evidence.page_index_node_id"
    )
    evidence_by_chunk: dict[str, tuple[str, str]] = {}
    while result.has_next():
        semantic_chunk_id, evidence_id, page_index_node_id = result.get_next()
        evidence_by_chunk[str(semantic_chunk_id)] = (str(evidence_id), str(page_index_node_id))
    return evidence_by_chunk


def _page_index_node_id_from_chunk(semantic_chunk_id: str) -> str:
    suffix = ":chunk-"
    if suffix in semantic_chunk_id:
        return semantic_chunk_id.split(suffix, 1)[0]
    return semantic_chunk_id


__all__ = [
    "HybridRetrievalMode",
    "HybridRetrievalQuery",
    "HybridRetrievalResponse",
    "InMemoryVectorCandidateIndex",
    "retrieve_hybrid",
]
