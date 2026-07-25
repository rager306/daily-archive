"""Read-only graph operators O1–O6 for SymFSM agent loop (M208).

All operators are allowlisted read tools over GraphReadPort and existing hybrid
retrieval. No write tools, no promotion authority, no open-ended Cypher.
SafetyFlags remain fail-closed.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any, Literal, Protocol

from research_graph.domain.universal_kb.contracts import SafetyFlags
from research_graph.infrastructure.retrieval.hybrid import (
    HybridRetrievalMode,
    HybridRetrievalQuery,
    InMemoryVectorCandidateIndex,
    retrieve_hybrid,
)

OperatorId = Literal["O1", "O2", "O3", "O4", "O5", "O6"]
ALLOWLISTED_OPERATORS: tuple[OperatorId, ...] = ("O1", "O2", "O3", "O4", "O5", "O6")
FORBIDDEN_OPERATORS: frozenset[str] = frozenset(
    {
        "write_graph",
        "upsert_scientific_kg",
        "promote",
        "import_eligible_true",
        "open_cypher",
        "execute_arbitrary",
        "init_schema",
    }
)


class GraphReadLike(Protocol):
    def seed_match(self, needle: str, *, limit: int = 10) -> list[dict[str, Any]]: ...

    def lineage_expand(self, needle: str, *, limit: int = 10) -> list[dict[str, Any]]: ...

    def evidence_paths_by_chunk(self) -> dict[str, tuple[str, str]]: ...

    def page_neighbors(self, semantic_chunk_id: str) -> list[str]: ...

    def integrity_scan(self) -> dict[str, Any]: ...


@dataclass(frozen=True, slots=True)
class GraphRef:
    """Bounded typed graph reference (metadata only)."""

    ref_id: str
    ref_type: str  # paper|entity|claim|chunk|evidence
    semantic_chunk_id: str | None = None
    evidence_path_id: str | None = None
    score: float = 0.0
    source_kind: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "ref_id": self.ref_id,
            "ref_type": self.ref_type,
            "semantic_chunk_id": self.semantic_chunk_id,
            "evidence_path_id": self.evidence_path_id,
            "score": self.score,
            "source_kind": self.source_kind,
        }


@dataclass(frozen=True, slots=True)
class OperatorResult:
    operator: OperatorId
    refs: tuple[GraphRef, ...]
    ambiguity: tuple[str, ...] = ()
    diagnostics: tuple[str, ...] = ()
    bounded: bool = True
    limit: int = 0
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if not self.bounded:
            raise ValueError("operators must return bounded results")

    def to_dict(self) -> dict[str, Any]:
        return {
            "operator": self.operator,
            "refs": [r.to_dict() for r in self.refs],
            "ambiguity": list(self.ambiguity),
            "diagnostics": list(self.diagnostics),
            "bounded": self.bounded,
            "limit": self.limit,
            "safety_flags": self.safety_flags.to_dict(),
        }


def _ref_from_row(row: Mapping[str, Any], *, ref_type: str = "chunk") -> GraphRef:
    chunk = str(row.get("semantic_chunk_id") or "")
    evidence = str(row.get("evidence_path_id") or "")
    score = float(row.get("graph_score") or row.get("score") or 0.0)
    ref_id = evidence or chunk or str(row.get("page_index_node_id") or "unknown")
    return GraphRef(
        ref_id=ref_id,
        ref_type=ref_type,
        semantic_chunk_id=chunk or None,
        evidence_path_id=evidence or None,
        score=score,
        source_kind=str(row.get("source_kind") or row.get("graph_source") or "") or None,
    )


def o1_resolve_seed(
    graph_read: GraphReadLike,
    seed: str,
    *,
    limit: int = 5,
    vector_index: InMemoryVectorCandidateIndex | None = None,
    query_vector: tuple[float, ...] | None = None,
) -> OperatorResult:
    """O1: resolve free-text/entity/claim seed to bounded typed graph refs."""
    needle = seed.strip()
    if not needle:
        return OperatorResult(
            operator="O1",
            refs=(),
            ambiguity=("empty_seed",),
            diagnostics=("empty_seed",),
            limit=limit,
        )
    rows = graph_read.seed_match(needle, limit=limit)
    refs = tuple(_ref_from_row(r, ref_type="chunk") for r in rows)
    # Optional vector assist for free-text (still read-only, bounded).
    if vector_index is not None and query_vector is not None and len(refs) < limit:
        response = retrieve_hybrid(
            query=HybridRetrievalQuery(
                text=needle,
                vector=query_vector,
                mode=HybridRetrievalMode.HYBRID,
                limit=limit,
            ),
            graph_read=graph_read,  # type: ignore[arg-type]
            vector_index=vector_index,
        )
        extra = [
            GraphRef(
                ref_id=str(r.get("evidence_path_id") or r.get("semantic_chunk_id")),
                ref_type="chunk",
                semantic_chunk_id=str(r.get("semantic_chunk_id") or "") or None,
                evidence_path_id=str(r.get("evidence_path_id") or "") or None,
                score=float(r.get("fusion_score") or 0.0),
                source_kind="vector+graph",
            )
            for r in response.results
        ]
        # merge unique by ref_id
        by_id = {r.ref_id: r for r in refs}
        for e in extra:
            by_id.setdefault(e.ref_id, e)
        refs = tuple(list(by_id.values())[:limit])

    ambiguity: list[str] = []
    if len(refs) > 1:
        ambiguity.append(f"multi_match:{len(refs)}")
    if not refs:
        ambiguity.append("no_match")
    return OperatorResult(
        operator="O1",
        refs=refs,
        ambiguity=tuple(ambiguity),
        diagnostics=("o1_seed_resolution", f"match_count:{len(refs)}"),
        limit=limit,
    )


def o2_citation_lineage(
    graph_read: GraphReadLike,
    seed_ref: GraphRef | str,
    *,
    limit: int = 8,
) -> OperatorResult:
    """O2: expand bounded citation/evidence lineage from resolved seed."""
    needle = seed_ref.semantic_chunk_id if isinstance(seed_ref, GraphRef) else str(seed_ref)
    if isinstance(seed_ref, GraphRef) and seed_ref.evidence_path_id:
        needle = seed_ref.semantic_chunk_id or seed_ref.ref_id
    needle = (needle or str(seed_ref)).strip()
    rows = graph_read.lineage_expand(needle, limit=limit)
    evidence = graph_read.evidence_paths_by_chunk()
    refs: list[GraphRef] = []
    for row in rows:
        ref = _ref_from_row(row, ref_type="lineage")
        if ref.semantic_chunk_id and not ref.evidence_path_id:
            ep, _page = evidence.get(ref.semantic_chunk_id, ("", ""))
            if ep:
                ref = GraphRef(
                    ref_id=ep,
                    ref_type="lineage",
                    semantic_chunk_id=ref.semantic_chunk_id,
                    evidence_path_id=ep,
                    score=ref.score,
                    source_kind=ref.source_kind,
                )
        refs.append(ref)
    return OperatorResult(
        operator="O2",
        refs=tuple(refs[:limit]),
        diagnostics=("o2_citation_lineage", f"lineage_count:{len(refs)}"),
        limit=limit,
    )


def o3_method_neighborhood(
    graph_read: GraphReadLike,
    seed_ref: GraphRef | str,
    *,
    limit: int = 8,
) -> OperatorResult:
    """O3: method/section neighborhood via page neighbors + seed match."""
    chunk_id = (
        seed_ref.semantic_chunk_id
        if isinstance(seed_ref, GraphRef)
        else str(seed_ref)
    ) or ""
    neighbors = graph_read.page_neighbors(chunk_id) if chunk_id else []
    evidence = graph_read.evidence_paths_by_chunk()
    refs: list[GraphRef] = []
    for nid in neighbors[:limit]:
        ep, page = evidence.get(nid, ("", ""))
        refs.append(
            GraphRef(
                ref_id=ep or nid,
                ref_type="neighborhood",
                semantic_chunk_id=nid,
                evidence_path_id=ep or None,
                score=0.7,
                source_kind="method_neighborhood",
            )
        )
    # diversity: unique page prefixes
    pages = { (r.semantic_chunk_id or "").split(":")[1] if r.semantic_chunk_id and ":" in r.semantic_chunk_id else "unknown" for r in refs }
    return OperatorResult(
        operator="O3",
        refs=tuple(refs),
        diagnostics=(
            "o3_method_neighborhood",
            f"neighbor_count:{len(refs)}",
            f"source_diversity:{len(pages)}",
        ),
        limit=limit,
    )


def o4_topic_neighborhood(
    graph_read: GraphReadLike,
    topic: str,
    *,
    limit: int = 8,
) -> OperatorResult:
    """O4: topic neighborhood (typed bounded subgraph metadata)."""
    rows = graph_read.seed_match(topic, limit=limit)
    lineage = graph_read.lineage_expand(topic, limit=limit)
    by_id: dict[str, GraphRef] = {}
    for row in rows + lineage:
        ref = _ref_from_row(row, ref_type="topic")
        by_id.setdefault(ref.ref_id, ref)
    refs = list(by_id.values())[:limit]
    sources = {r.source_kind or "unknown" for r in refs}
    return OperatorResult(
        operator="O4",
        refs=tuple(refs),
        diagnostics=(
            "o4_topic_neighborhood",
            f"topic_ref_count:{len(refs)}",
            f"source_diversity:{len(sources)}",
        ),
        limit=limit,
    )


def o5_gap_detection(
    graph_read: GraphReadLike,
    subgraph_refs: Sequence[GraphRef],
    *,
    expected_chunk_ids: Sequence[str] = (),
) -> OperatorResult:
    """O5: explicit evidence gaps/contradictions/unsupported edges (no mutation)."""
    integrity = graph_read.integrity_scan()
    present = {r.semantic_chunk_id for r in subgraph_refs if r.semantic_chunk_id}
    present_ev = {r.evidence_path_id for r in subgraph_refs if r.evidence_path_id}
    missing_expected = [c for c in expected_chunk_ids if c not in present]
    gaps: list[GraphRef] = []
    for mid in missing_expected:
        gaps.append(
            GraphRef(
                ref_id=f"gap:missing_chunk:{mid}",
                ref_type="gap",
                semantic_chunk_id=mid,
                score=0.0,
                source_kind="gap",
            )
        )
    broken = int(integrity.get("broken_evidence_paths") or 0)
    orphans = int(integrity.get("orphan_evidence_chunks") or 0)
    if broken:
        gaps.append(
            GraphRef(
                ref_id=f"gap:broken_evidence:{broken}",
                ref_type="gap",
                score=0.0,
                source_kind="integrity",
            )
        )
    if orphans:
        gaps.append(
            GraphRef(
                ref_id=f"gap:orphan_chunks:{orphans}",
                ref_type="gap",
                score=0.0,
                source_kind="integrity",
            )
        )
    # unsupported edges: refs without evidence_path
    unsupported = [r for r in subgraph_refs if not r.evidence_path_id]
    for u in unsupported[:3]:
        gaps.append(
            GraphRef(
                ref_id=f"gap:unsupported:{u.ref_id}",
                ref_type="gap",
                semantic_chunk_id=u.semantic_chunk_id,
                score=0.0,
                source_kind="unsupported_edge",
            )
        )
    return OperatorResult(
        operator="O5",
        refs=tuple(gaps),
        diagnostics=(
            "o5_gap_detection",
            f"missing_expected:{len(missing_expected)}",
            f"broken_evidence:{broken}",
            f"unsupported:{len(unsupported)}",
            f"present_evidence:{len(present_ev)}",
        ),
        limit=len(gaps),
    )


def o6_related_source_discovery(
    graph_read: GraphReadLike,
    gap_refs: Sequence[GraphRef],
    *,
    vector_index: InMemoryVectorCandidateIndex | None = None,
    query_vector: tuple[float, ...] | None = None,
    limit: int = 5,
) -> OperatorResult:
    """O6: bounded ranked source suggestions from existing retrieval only."""
    needles = [
        (g.semantic_chunk_id or g.ref_id).replace("gap:missing_chunk:", "")
        for g in gap_refs
    ]
    needle = next((n for n in needles if n and not n.startswith("gap:")), "PageIndex")
    mode = HybridRetrievalMode.HYBRID if vector_index and query_vector else HybridRetrievalMode.GRAPH_ONLY
    response = retrieve_hybrid(
        query=HybridRetrievalQuery(
            text=needle,
            vector=query_vector,
            mode=mode,
            limit=limit,
        ),
        graph_read=graph_read,  # type: ignore[arg-type]
        vector_index=vector_index,
    )
    refs = tuple(
        GraphRef(
            ref_id=str(r.get("evidence_path_id") or r.get("semantic_chunk_id")),
            ref_type="source_suggestion",
            semantic_chunk_id=str(r.get("semantic_chunk_id") or "") or None,
            evidence_path_id=str(r.get("evidence_path_id") or "") or None,
            score=float(r.get("fusion_score") or r.get("graph_score") or 0.0),
            source_kind="retrieval",
        )
        for r in response.results[:limit]
    )
    return OperatorResult(
        operator="O6",
        refs=refs,
        diagnostics=("o6_related_source_discovery", f"suggestions:{len(refs)}", f"needle:{needle[:40]}"),
        limit=limit,
    )


def assert_operator_allowlisted(operator_id: str) -> None:
    if operator_id not in ALLOWLISTED_OPERATORS:
        raise PermissionError(f"operator_not_allowlisted:{operator_id}")
    if operator_id in FORBIDDEN_OPERATORS:
        raise PermissionError(f"operator_forbidden:{operator_id}")


__all__ = [
    "ALLOWLISTED_OPERATORS",
    "FORBIDDEN_OPERATORS",
    "GraphRef",
    "OperatorId",
    "OperatorResult",
    "assert_operator_allowlisted",
    "o1_resolve_seed",
    "o2_citation_lineage",
    "o3_method_neighborhood",
    "o4_topic_neighborhood",
    "o5_gap_detection",
    "o6_related_source_discovery",
]
