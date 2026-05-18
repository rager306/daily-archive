"""Deterministic fixture-level RLM graph traversal comparisons.

The boundary in this module is deliberately local-only and read-only: callers
provide a LadybugDB fixture handle plus an in-memory candidate index, and the
module returns ID/count/score diagnostics suitable for contract tests and logs.
"""

from __future__ import annotations

from collections import Counter, deque
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass, field
from enum import StrEnum
from typing import Any, cast

from arxiv_archive.evaluation import (
    calculate_evidence_path_hit_rate,
    calculate_retrieval_recall,
)
from arxiv_archive.hybrid_retrieval import (
    HybridRetrievalMode,
    HybridRetrievalQuery,
    InMemoryVectorCandidateIndex,
    retrieve_hybrid,
)


class TraversalPolicyLabel(StrEnum):
    """Supported deterministic traversal policies."""

    RLM_STYLE_DETERMINISTIC = "rlm_style_deterministic"
    HEURISTIC_BFS = "heuristic_bfs"


@dataclass(frozen=True)
class RLMGraphTraversalPolicy:
    """Policy selector for the local traversal harness."""

    label: TraversalPolicyLabel = TraversalPolicyLabel.RLM_STYLE_DETERMINISTIC


@dataclass(frozen=True)
class RLMGraphTraversalConfig:
    """Bounded traversal controls."""

    max_steps: int = 4
    max_neighbors_per_step: int = 3
    top_k: int = 4

    def __post_init__(self) -> None:
        if self.max_steps < 0:
            raise ValueError("invalid_config:max_steps_negative")
        if self.max_neighbors_per_step < 0:
            raise ValueError("invalid_config:max_neighbors_per_step_negative")
        if self.top_k < 0:
            raise ValueError("invalid_config:top_k_negative")


@dataclass(frozen=True)
class RLMGraphTraversalQuestion:
    """One scattered-evidence benchmark question."""

    name: str
    query: str = field(repr=False)
    query_vector: tuple[float, ...] | None = field(default=None, repr=False)
    seed_semantic_chunk_ids: tuple[str, ...] = ()
    seed_evidence_path_ids: tuple[str, ...] = ()
    expected_semantic_chunk_ids: Iterable[str] = field(default_factory=frozenset)
    expected_evidence_path_ids: Iterable[str] = field(default_factory=frozenset)
    optimizer_hint: str | None = field(default=None, repr=False)


@dataclass(frozen=True)
class RouteStep:
    """One ID-only traversal route step."""

    step_index: int
    from_id: str | None
    to_id: str | None
    action: str
    score: float
    status: str
    depth: int = 0
    source_edge_label: str | None = None
    evidence_path_id: str | None = None
    stop_reason: str | None = None


@dataclass(frozen=True)
class ReturnedCandidate:
    """Traceable returned candidate without source text or vector values."""

    semantic_chunk_id: str
    page_index_node_id: str | None
    evidence_path_id: str | None
    route_policy: str
    depth: int
    score_metadata: Mapping[str, float]


@dataclass(frozen=True)
class TraversalMetrics:
    """ID-only retrieval metrics for one traversal or baseline."""

    retrieval_recall: float
    evidence_path_hit_rate: float
    missing_expected_result_ids: list[str]
    missing_expected_evidence_path_ids: list[str]


@dataclass(frozen=True)
class TraversalResult:
    """Typed result for one traversal policy."""

    question_id: str
    policy_label: str
    trajectory: tuple[RouteStep, ...]
    candidates: tuple[ReturnedCandidate, ...]
    visited_semantic_chunk_ids: list[str]
    returned_semantic_chunk_ids: list[str]
    returned_evidence_path_ids: list[str]
    metrics: TraversalMetrics
    stop_reason: str
    budget_exhausted: bool
    diagnostics: dict[str, Any]


@dataclass(frozen=True)
class BaselineResult:
    """Typed result for one non-RLM baseline."""

    question_id: str
    label: str
    returned_semantic_chunk_ids: list[str]
    returned_evidence_path_ids: list[str]
    metrics: TraversalMetrics
    source_diagnostics: dict[str, Any]


@dataclass(frozen=True)
class ComparisonResult:
    """Full comparison for one question."""

    question_id: str
    config: RLMGraphTraversalConfig
    rlm_traversal: TraversalResult
    baselines: tuple[BaselineResult, ...]
    diagnostics: dict[str, Any]


def compare_rlm_graph_traversal(
    conn: Any,
    question: RLMGraphTraversalQuestion,
    *,
    vector_index: InMemoryVectorCandidateIndex,
    config: RLMGraphTraversalConfig | None = None,
    policy: RLMGraphTraversalPolicy | None = None,
) -> ComparisonResult:
    """Compare deterministic RLM-style traversal with local retrieval baselines."""
    resolved_config = config or RLMGraphTraversalConfig()
    resolved_policy = policy or RLMGraphTraversalPolicy()
    if not question.seed_semantic_chunk_ids:
        raise ValueError(f"empty_seed:{question.name}")

    evidence_by_chunk = _evidence_paths_by_chunk(conn)
    rlm_result = _run_rlm_style_traversal(
        conn,
        question,
        vector_index=vector_index,
        config=resolved_config,
        policy=resolved_policy,
        evidence_by_chunk=evidence_by_chunk,
    )
    baselines = (
        _run_vector_only_baseline(conn, question, vector_index, resolved_config, evidence_by_chunk),
        _run_graph_one_hop_baseline(conn, question, resolved_config),
        _run_hybrid_baseline(conn, question, vector_index, resolved_config),
        _run_heuristic_bfs_baseline(conn, question, resolved_config, evidence_by_chunk),
    )
    diagnostics = {
        "question_id": question.name,
        "policy_label": rlm_result.policy_label,
        "visited_count": len(rlm_result.visited_semantic_chunk_ids),
        "returned_count": len(rlm_result.returned_semantic_chunk_ids),
        "baseline_labels": [baseline.label for baseline in baselines],
        "stop_reason": rlm_result.stop_reason,
        "budget_exhausted": rlm_result.budget_exhausted,
        "missing_expected_result_ids": rlm_result.metrics.missing_expected_result_ids,
        "missing_expected_evidence_path_ids": rlm_result.metrics.missing_expected_evidence_path_ids,
    }
    return ComparisonResult(
        question_id=question.name,
        config=resolved_config,
        rlm_traversal=rlm_result,
        baselines=baselines,
        diagnostics=diagnostics,
    )


def _run_rlm_style_traversal(
    conn: Any,
    question: RLMGraphTraversalQuestion,
    *,
    vector_index: InMemoryVectorCandidateIndex,
    config: RLMGraphTraversalConfig,
    policy: RLMGraphTraversalPolicy,
    evidence_by_chunk: Mapping[str, tuple[str, str]],
) -> TraversalResult:
    route: list[RouteStep] = []
    visited: list[str] = []
    candidates: list[ReturnedCandidate] = []
    seen_candidate_ids: list[str] = []
    depths: dict[str, int] = {}
    score_by_id: dict[str, float] = {}

    for seed_id in question.seed_semantic_chunk_ids:
        _record_candidate(
            seed_id,
            candidates=candidates,
            seen_candidate_ids=seen_candidate_ids,
            evidence_by_chunk=evidence_by_chunk,
            policy_label=policy.label.value,
            depth=0,
            score_metadata={"seed_score": 1.0},
        )
        if seed_id not in visited:
            visited.append(seed_id)
        depths[seed_id] = 0
        route.append(
            RouteStep(
                step_index=len(route),
                from_id=None,
                to_id=seed_id,
                action="seed",
                score=1.0,
                status="selected",
                depth=0,
                evidence_path_id=evidence_by_chunk.get(seed_id, (None, None))[0],
            )
        )

    if config.max_steps == 0:
        return _build_traversal_result(
            question,
            policy_label=policy.label.value,
            route=route,
            candidates=candidates,
            seen_candidate_ids=seen_candidate_ids,
            visited=visited,
            stop_reason="budget_exhausted",
            budget_exhausted=True,
        )

    vector_diagnostics: dict[str, Any] = {}
    vector_rows = _vector_rows(
        question,
        vector_index,
        config,
        evidence_by_chunk,
        diagnostics=vector_diagnostics,
    )
    graph_rows = _graph_rows(conn, question.query, limit=config.top_k)
    scored_rows = _merge_scored_rows(vector_rows, graph_rows)
    for row in scored_rows:
        chunk_id = cast(str, row["semantic_chunk_id"])
        if chunk_id in question.seed_semantic_chunk_ids:
            seen_candidate_ids.append(chunk_id)
    frontier = deque(seed_id for seed_id in question.seed_semantic_chunk_ids)
    steps_used = 0
    empty_neighborhood = not scored_rows

    while frontier and steps_used < config.max_steps:
        current_id = frontier.popleft()
        current_depth = depths.get(current_id, 0)
        neighbors = [row for row in scored_rows if row["semantic_chunk_id"] != current_id]
        neighbors.sort(
            key=lambda row: (
                -cast(float, row["score"]),
                cast(str, row["semantic_chunk_id"]),
            )
        )
        neighbors = neighbors[: config.max_neighbors_per_step]
        if not neighbors:
            continue
        for row in neighbors:
            if steps_used >= config.max_steps:
                break
            chunk_id = cast(str, row["semantic_chunk_id"])
            row_depth = current_depth + 1
            score = cast(float, row["score"])
            score_by_id[chunk_id] = score
            _record_candidate(
                chunk_id,
                candidates=candidates,
                seen_candidate_ids=seen_candidate_ids,
                evidence_by_chunk=evidence_by_chunk,
                policy_label=policy.label.value,
                depth=row_depth,
                score_metadata={"selection_score": score},
                page_index_node_id=cast(str | None, row.get("page_index_node_id")),
                evidence_path_id=cast(str | None, row.get("evidence_path_id")),
            )
            if chunk_id not in visited:
                visited.append(chunk_id)
                depths[chunk_id] = row_depth
                frontier.append(chunk_id)
            route.append(
                RouteStep(
                    step_index=len(route),
                    from_id=current_id,
                    to_id=chunk_id,
                    action="expand_neighbor",
                    score=score,
                    status="selected",
                    depth=row_depth,
                    source_edge_label=cast(str | None, row.get("source")),
                    evidence_path_id=cast(str | None, row.get("evidence_path_id")),
                )
            )
            steps_used += 1
            if _has_expected_ids(question, candidates):
                route.append(
                    RouteStep(
                        step_index=len(route),
                        from_id=chunk_id,
                        to_id=None,
                        action="stop",
                        score=score_by_id.get(chunk_id, 0.0),
                        status="target_recall_reached",
                        depth=row_depth,
                        stop_reason="target_recall_reached",
                    )
                )
                return _build_traversal_result(
                    question,
                    policy_label=policy.label.value,
                    route=route,
                    candidates=candidates,
                    seen_candidate_ids=seen_candidate_ids,
                    visited=visited,
                    stop_reason="target_recall_reached",
                    budget_exhausted=False,
                    extra_diagnostics=vector_diagnostics,
                )

    stop_reason = "empty_neighborhood" if empty_neighborhood else "budget_exhausted"
    return _build_traversal_result(
        question,
        policy_label=policy.label.value,
        route=route,
        candidates=candidates,
        seen_candidate_ids=seen_candidate_ids,
        visited=visited,
        stop_reason=stop_reason,
        budget_exhausted=stop_reason == "budget_exhausted",
        extra_diagnostics=vector_diagnostics,
    )


def _run_vector_only_baseline(
    conn: Any,
    question: RLMGraphTraversalQuestion,
    vector_index: InMemoryVectorCandidateIndex,
    config: RLMGraphTraversalConfig,
    evidence_by_chunk: Mapping[str, tuple[str, str]],
) -> BaselineResult:
    try:
        response = retrieve_hybrid(
            conn,
            HybridRetrievalQuery(
                text=question.query,
                vector=question.query_vector,
                mode=HybridRetrievalMode.VECTOR_ONLY,
                limit=config.top_k,
            ),
            vector_index=vector_index,
        )
        rows = [dict(row, score=float(row.get("fusion_score") or 0.0)) for row in response.results]
        diagnostics = _safe_diagnostics(response.diagnostics)
        diagnostics["mode"] = "vector_only"
    except ValueError as exc:
        rows = []
        diagnostics = _typed_error_diagnostics("vector_only", exc)
    if not rows and "empty_candidate_index" not in diagnostics:
        diagnostics["empty_candidate_index"] = True
    return _baseline_from_rows(
        question,
        label="vector_only",
        rows=rows,
        source_diagnostics=diagnostics,
    )


def _run_graph_one_hop_baseline(
    conn: Any,
    question: RLMGraphTraversalQuestion,
    config: RLMGraphTraversalConfig,
) -> BaselineResult:
    rows = _graph_rows(conn, question.query, limit=config.top_k)
    return _baseline_from_rows(
        question,
        label="graph_one_hop",
        rows=rows,
        source_diagnostics={"mode": "graph_one_hop", "candidate_count": len(rows)},
    )


def _run_hybrid_baseline(
    conn: Any,
    question: RLMGraphTraversalQuestion,
    vector_index: InMemoryVectorCandidateIndex,
    config: RLMGraphTraversalConfig,
) -> BaselineResult:
    try:
        response = retrieve_hybrid(
            conn,
            HybridRetrievalQuery(
                text=question.query,
                vector=question.query_vector,
                mode=HybridRetrievalMode.HYBRID,
                limit=config.top_k,
            ),
            vector_index=vector_index,
        )
        rows = [dict(row, score=float(row.get("fusion_score") or 0.0)) for row in response.results]
        diagnostics = _safe_diagnostics(response.diagnostics)
        diagnostics["mode"] = "hybrid"
    except ValueError as exc:
        rows = []
        diagnostics = _typed_error_diagnostics("hybrid", exc)
    if not rows and "empty_candidate_index" not in diagnostics:
        diagnostics["empty_candidate_index"] = True
    return _baseline_from_rows(question, label="hybrid", rows=rows, source_diagnostics=diagnostics)


def _run_heuristic_bfs_baseline(
    conn: Any,
    question: RLMGraphTraversalQuestion,
    config: RLMGraphTraversalConfig,
    evidence_by_chunk: Mapping[str, tuple[str, str]],
) -> BaselineResult:
    visited: list[str] = []
    queue = deque(question.seed_semantic_chunk_ids)
    while queue and len(visited) < max(config.top_k, 0):
        chunk_id = queue.popleft()
        if chunk_id in visited:
            continue
        visited.append(chunk_id)
        for neighbor in _page_neighbors(conn, chunk_id, evidence_by_chunk):
            if neighbor not in visited and neighbor not in queue:
                queue.append(neighbor)
    rows = [_row_for_chunk(chunk_id, evidence_by_chunk, score=1.0) for chunk_id in visited]
    return _baseline_from_rows(
        question,
        label="heuristic_bfs",
        rows=rows,
        source_diagnostics={
            "mode": "heuristic_bfs",
            "visited_count": len(visited),
            "budget_exhausted": bool(queue),
        },
    )


def _vector_rows(
    question: RLMGraphTraversalQuestion,
    vector_index: InMemoryVectorCandidateIndex,
    config: RLMGraphTraversalConfig,
    evidence_by_chunk: Mapping[str, tuple[str, str]],
    *,
    diagnostics: dict[str, Any] | None = None,
) -> list[dict[str, Any]]:
    if question.query_vector is None:
        if diagnostics is not None:
            diagnostics["empty_vector_candidates"] = True
            diagnostics["empty_candidate_index"] = True
        return []
    rows: list[dict[str, Any]] = []
    try:
        vector_candidates = vector_index.search(question.query_vector, limit=config.top_k)
    except ValueError as exc:
        if diagnostics is not None:
            diagnostics.update(_typed_error_diagnostics("vector", exc))
        return []
    for candidate in vector_candidates:
        evidence_path_id, page_index_node_id = evidence_by_chunk.get(
            candidate.semantic_chunk_id,
            (None, _page_index_node_id_from_chunk(candidate.semantic_chunk_id)),
        )
        rows.append(
            {
                "semantic_chunk_id": candidate.semantic_chunk_id,
                "page_index_node_id": page_index_node_id,
                "evidence_path_id": evidence_path_id,
                "score": float(candidate.vector_score),
                "source": "vector",
            }
        )
    if diagnostics is not None and not rows:
        diagnostics["empty_vector_candidates"] = True
        diagnostics["empty_candidate_index"] = True
    return rows


def _graph_rows(conn: Any, text: str, *, limit: int) -> list[dict[str, Any]]:
    response = retrieve_hybrid(
        conn,
        HybridRetrievalQuery(
            text=text, vector=None, mode=HybridRetrievalMode.GRAPH_ONLY, limit=limit
        ),
    )
    return [
        dict(row, score=float(row.get("graph_score") or 0.0), source="graph")
        for row in response.results
    ]


def _merge_scored_rows(
    vector_rows: Sequence[Mapping[str, Any]],
    graph_rows: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    by_id: dict[str, dict[str, Any]] = {}
    for row in [*graph_rows, *vector_rows]:
        chunk_id = cast(str, row["semantic_chunk_id"])
        score = float(row.get("score") or 0.0)
        current = by_id.get(chunk_id)
        if current is None or score > float(current.get("score") or 0.0):
            by_id[chunk_id] = dict(row, score=score)
    return list(by_id.values())


def _record_candidate(
    semantic_chunk_id: str,
    *,
    candidates: list[ReturnedCandidate],
    seen_candidate_ids: list[str],
    evidence_by_chunk: Mapping[str, tuple[str, str]],
    policy_label: str,
    depth: int,
    score_metadata: Mapping[str, float],
    page_index_node_id: str | None = None,
    evidence_path_id: str | None = None,
) -> None:
    seen_candidate_ids.append(semantic_chunk_id)
    if any(candidate.semantic_chunk_id == semantic_chunk_id for candidate in candidates):
        return
    evidence = evidence_by_chunk.get(semantic_chunk_id)
    resolved_evidence_id = evidence_path_id or (evidence[0] if evidence is not None else None)
    resolved_page_id = page_index_node_id or (
        evidence[1] if evidence is not None else _page_index_node_id_from_chunk(semantic_chunk_id)
    )
    candidates.append(
        ReturnedCandidate(
            semantic_chunk_id=semantic_chunk_id,
            page_index_node_id=resolved_page_id,
            evidence_path_id=resolved_evidence_id,
            route_policy=policy_label,
            depth=depth,
            score_metadata=dict(score_metadata),
        )
    )


def _build_traversal_result(
    question: RLMGraphTraversalQuestion,
    *,
    policy_label: str,
    route: list[RouteStep],
    candidates: list[ReturnedCandidate],
    seen_candidate_ids: list[str],
    visited: list[str],
    stop_reason: str,
    budget_exhausted: bool,
    extra_diagnostics: Mapping[str, Any] | None = None,
) -> TraversalResult:
    rows = _candidate_rows(candidates)
    metrics = _metrics(question, rows)
    duplicate_ids = sorted(id_ for id_, count in Counter(seen_candidate_ids).items() if count > 1)
    diagnostics = {
        "stop_reason": stop_reason,
        "budget_exhausted": budget_exhausted,
        "visited_count": len(visited),
        "returned_count": len(candidates),
        "duplicate_candidate_ids": duplicate_ids,
        "empty_neighborhood": stop_reason == "empty_neighborhood",
        "missing_expected_result_ids": metrics.missing_expected_result_ids,
        "missing_expected_evidence_path_ids": metrics.missing_expected_evidence_path_ids,
    }
    if extra_diagnostics:
        diagnostics.update(_safe_diagnostics(extra_diagnostics))
    return TraversalResult(
        question_id=question.name,
        policy_label=policy_label,
        trajectory=tuple(route),
        candidates=tuple(candidates),
        visited_semantic_chunk_ids=list(visited),
        returned_semantic_chunk_ids=sorted(
            {candidate.semantic_chunk_id for candidate in candidates}
        ),
        returned_evidence_path_ids=sorted(
            {candidate.evidence_path_id for candidate in candidates if candidate.evidence_path_id}
        ),
        metrics=metrics,
        stop_reason=stop_reason,
        budget_exhausted=budget_exhausted,
        diagnostics=diagnostics,
    )


def _baseline_from_rows(
    question: RLMGraphTraversalQuestion,
    *,
    label: str,
    rows: Sequence[Mapping[str, Any]],
    source_diagnostics: dict[str, Any],
) -> BaselineResult:
    metrics = _metrics(question, rows)
    return BaselineResult(
        question_id=question.name,
        label=label,
        returned_semantic_chunk_ids=metrics_returned_ids(rows, "semantic_chunk_id"),
        returned_evidence_path_ids=metrics_returned_ids(rows, "evidence_path_id"),
        metrics=metrics,
        source_diagnostics=_safe_diagnostics(source_diagnostics),
    )


def _metrics(
    question: RLMGraphTraversalQuestion, rows: Iterable[Mapping[str, Any]]
) -> TraversalMetrics:
    row_list = list(rows)
    recall = calculate_retrieval_recall(
        row_list,
        question.expected_semantic_chunk_ids,
        result_id_field="semantic_chunk_id",
    )
    hit_rate = calculate_evidence_path_hit_rate(row_list, question.expected_evidence_path_ids)
    return TraversalMetrics(
        retrieval_recall=recall.recall,
        evidence_path_hit_rate=hit_rate.hit_rate,
        missing_expected_result_ids=recall.missing_expected_result_ids,
        missing_expected_evidence_path_ids=hit_rate.missing_expected_evidence_path_ids,
    )


def metrics_returned_ids(rows: Sequence[Mapping[str, Any]], field_name: str) -> list[str]:
    return sorted({str(row[field_name]) for row in rows if row.get(field_name) is not None})


def _candidate_rows(candidates: Sequence[ReturnedCandidate]) -> list[dict[str, Any]]:
    return [
        {
            "semantic_chunk_id": candidate.semantic_chunk_id,
            "page_index_node_id": candidate.page_index_node_id,
            "evidence_path_id": candidate.evidence_path_id,
        }
        for candidate in candidates
    ]


def _has_expected_ids(
    question: RLMGraphTraversalQuestion,
    candidates: Sequence[ReturnedCandidate],
) -> bool:
    returned_chunk_ids = {candidate.semantic_chunk_id for candidate in candidates}
    returned_evidence_ids = {candidate.evidence_path_id for candidate in candidates}
    return (
        set(question.expected_semantic_chunk_ids) <= returned_chunk_ids
        and set(question.expected_evidence_path_ids) <= returned_evidence_ids
    )


def _evidence_paths_by_chunk(conn: Any) -> dict[str, tuple[str, str]]:
    result = conn.execute(
        "MATCH (evidence:EvidencePath) "
        "RETURN evidence.semantic_chunk_id, evidence.id, evidence.page_index_node_id"
    )
    rows: dict[str, tuple[str, str]] = {}
    while result.has_next():
        semantic_chunk_id, evidence_path_id, page_index_node_id = result.get_next()
        rows[str(semantic_chunk_id)] = (str(evidence_path_id), str(page_index_node_id))
    return rows


def _page_neighbors(
    conn: Any,
    semantic_chunk_id: str,
    evidence_by_chunk: Mapping[str, tuple[str, str]],
) -> list[str]:
    page_id = evidence_by_chunk.get(
        semantic_chunk_id,
        ("", _page_index_node_id_from_chunk(semantic_chunk_id)),
    )[1]
    if not page_id:
        return []
    result = conn.execute(
        "MATCH (node:PageIndexNode)-[:NEXT_PAGE_INDEX_NODE]->(next:PageIndexNode), "
        "(next)-[:HAS_SEMANTIC_CHUNK]->(chunk:SemanticChunk) "
        "RETURN node.id, chunk.id"
    )
    neighbors: list[str] = []
    while result.has_next():
        node_id, chunk_id = result.get_next()
        if str(node_id) == page_id:
            neighbors.append(str(chunk_id))
    return sorted(neighbors)


def _row_for_chunk(
    semantic_chunk_id: str,
    evidence_by_chunk: Mapping[str, tuple[str, str]],
    *,
    score: float,
) -> dict[str, Any]:
    evidence_path_id, page_index_node_id = evidence_by_chunk.get(
        semantic_chunk_id,
        (None, _page_index_node_id_from_chunk(semantic_chunk_id)),
    )
    return {
        "semantic_chunk_id": semantic_chunk_id,
        "page_index_node_id": page_index_node_id,
        "evidence_path_id": evidence_path_id,
        "score": score,
    }


def _safe_diagnostics(diagnostics: Mapping[str, Any]) -> dict[str, Any]:
    safe: dict[str, Any] = {}
    for key, value in diagnostics.items():
        if key in {"query_text", "text"}:
            continue
        safe[str(key)] = value
    return safe


def _typed_error_diagnostics(mode: str, exc: ValueError) -> dict[str, Any]:
    message = str(exc)
    error_type = "invalid_query_vector" if "vector" in message else "retrieval_error"
    return {
        "mode": mode,
        "status": "error",
        "error_type": error_type,
        "error_code": message.replace(" ", "_"),
        "empty_candidate_index": True,
    }


class _NullGraphConnection:
    """Read-only placeholder for S06 vector-only retrieval, which never queries graph rows."""

    def execute(self, query: str, params: dict[str, Any] | None = None) -> Any:
        del query, params
        raise AssertionError("vector_only baseline must not execute graph queries")


def _page_index_node_id_from_chunk(semantic_chunk_id: str) -> str:
    suffix = ":chunk-"
    if suffix in semantic_chunk_id:
        return semantic_chunk_id.split(suffix, 1)[0]
    return semantic_chunk_id


__all__ = [
    "BaselineResult",
    "ComparisonResult",
    "RLMGraphTraversalConfig",
    "RLMGraphTraversalPolicy",
    "RLMGraphTraversalQuestion",
    "RouteStep",
    "ReturnedCandidate",
    "TraversalMetrics",
    "TraversalPolicyLabel",
    "TraversalResult",
    "compare_rlm_graph_traversal",
]
