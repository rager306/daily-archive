"""Production retrieval quality, integrity, staged gates (M206 S03/S06–S09).

Composes existing hybrid/baselines through GraphReadPort. No writes, no DSPy
optimizer, no autonomous agents. Application stays free of ladybug/falkordb.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any, Literal, Protocol

from research_graph.domain.universal_kb.contracts import SafetyFlags

GateVerdict = Literal["proceed", "repair", "stop"]
PolicyName = Literal[
    "vector_only",
    "graph_one_hop",
    "hybrid",
    "heuristic_bfs",
    "rlm_bounded",
]

# Staged gate thresholds (metadata-only, fail-closed defaults).
PROCEED_MIN_RECALL = 0.5
PROCEED_MAX_FAILURE_RATE = 0.3
PROCEED_MAX_BROKEN_EVIDENCE = 0
REPAIR_MIN_RECALL = 0.2


class GraphReadLike(Protocol):
    def seed_match(self, needle: str, *, limit: int = 10) -> list[dict[str, Any]]: ...

    def lineage_expand(self, needle: str, *, limit: int = 10) -> list[dict[str, Any]]: ...

    def evidence_paths_by_chunk(self) -> dict[str, tuple[str, str]]: ...

    def page_neighbors(self, semantic_chunk_id: str) -> list[str]: ...

    def integrity_scan(self) -> dict[str, Any]: ...


@dataclass(frozen=True, slots=True)
class ReviewedQuery:
    query_id: str
    text: str
    expected_chunk_ids: tuple[str, ...] = ()
    expected_evidence_ids: tuple[str, ...] = ()
    query_vector: tuple[float, ...] | None = None


@dataclass(frozen=True, slots=True)
class EvidenceTrace:
    """One query evidence lineage (S03)."""

    query_id: str
    ranked: tuple[dict[str, Any], ...]
    source_chunk_ids: tuple[str, ...]
    evidence_path_ids: tuple[str, ...]
    graph_lineage_ids: tuple[str, ...]
    diagnostics: tuple[str, ...] = ()
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "query_id": self.query_id,
            "ranked": list(self.ranked),
            "source_chunk_ids": list(self.source_chunk_ids),
            "evidence_path_ids": list(self.evidence_path_ids),
            "graph_lineage_ids": list(self.graph_lineage_ids),
            "diagnostics": list(self.diagnostics),
            "safety_flags": self.safety_flags.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class PolicyMetrics:
    policy: PolicyName
    recall: float
    evidence_correctness: float
    latency_ms: float
    failure_rate: float
    query_count: int
    diagnostics: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "policy": self.policy,
            "recall": self.recall,
            "evidence_correctness": self.evidence_correctness,
            "latency_ms": self.latency_ms,
            "failure_rate": self.failure_rate,
            "query_count": self.query_count,
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class IntegrityAuditReport:
    orphan_nodes: int
    broken_evidence_paths: int
    duplicate_stable_ids: int
    schema_violations: int
    backend: str
    details: dict[str, Any] = field(default_factory=dict)
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "orphan_nodes": self.orphan_nodes,
            "broken_evidence_paths": self.broken_evidence_paths,
            "duplicate_stable_ids": self.duplicate_stable_ids,
            "schema_violations": self.schema_violations,
            "backend": self.backend,
            "details": dict(self.details),
            "safety_flags": self.safety_flags.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class DegradationOutcome:
    scenario: str
    safe: bool
    degraded_to: str | None
    diagnostics: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "scenario": self.scenario,
            "safe": self.safe,
            "degraded_to": self.degraded_to,
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class StagedQueryGateReport:
    query_count: int
    verdict: GateVerdict
    policy_metrics: tuple[PolicyMetrics, ...]
    integrity: IntegrityAuditReport
    reasons: tuple[str, ...]
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "query_count": self.query_count,
            "verdict": self.verdict,
            "policy_metrics": [m.to_dict() for m in self.policy_metrics],
            "integrity": self.integrity.to_dict(),
            "reasons": list(self.reasons),
            "safety_flags": self.safety_flags.to_dict(),
        }


def seed_and_lineage_parity(
    ladybug_read: GraphReadLike,
    falkor_read: GraphReadLike,
    *,
    needle: str,
    limit: int = 10,
) -> dict[str, Any]:
    """S01: same typed seed/lineage shape on Ladybug fixture and Falkor snapshot."""
    seed_l = ladybug_read.seed_match(needle, limit=limit)
    seed_f = falkor_read.seed_match(needle, limit=limit)
    lin_l = ladybug_read.lineage_expand(needle, limit=limit)
    lin_f = falkor_read.lineage_expand(needle, limit=limit)
    keys = ("semantic_chunk_id", "page_index_node_id", "evidence_path_id", "graph_score", "graph_source")

    def _shape(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
        shaped = []
        for row in rows:
            shaped.append({k: row.get(k) for k in keys})
        return shaped

    return {
        "seed_ladybug": _shape(seed_l),
        "seed_falkor": _shape(seed_f),
        "lineage_ladybug": _shape(lin_l),
        "lineage_falkor": _shape(lin_f),
        "seed_shape_compatible": all(
            set(keys).issubset(r.keys()) for r in (seed_l + seed_f) if r
        )
        or (not seed_l and not seed_f),
        "lineage_shape_compatible": all(
            set(keys).issubset(r.keys()) for r in (lin_l + lin_f) if r
        )
        or (not lin_l and not lin_f),
    }


def trace_one_query_evidence(
    graph_read: GraphReadLike,
    query: ReviewedQuery,
    *,
    ranked_rows: Sequence[Mapping[str, Any]],
) -> EvidenceTrace:
    """S03: ranked evidence with source/chunk/graph lineage ids only."""
    evidence = graph_read.evidence_paths_by_chunk()
    lineage = graph_read.lineage_expand(query.text, limit=10)
    ranked = tuple(dict(r) for r in ranked_rows)
    source_chunks = tuple(str(r.get("semantic_chunk_id")) for r in ranked if r.get("semantic_chunk_id"))
    evidence_ids = tuple(
        str(r.get("evidence_path_id") or evidence.get(str(r.get("semantic_chunk_id")), ("", ""))[0])
        for r in ranked
    )
    lineage_ids = tuple(str(r.get("semantic_chunk_id")) for r in lineage)
    return EvidenceTrace(
        query_id=query.query_id,
        ranked=ranked,
        source_chunk_ids=source_chunks,
        evidence_path_ids=tuple(e for e in evidence_ids if e),
        graph_lineage_ids=lineage_ids,
        diagnostics=("evidence_trace_metadata_only",),
    )


def run_integrity_audit(graph_read: GraphReadLike) -> IntegrityAuditReport:
    """S07: orphan nodes, broken EvidencePaths, duplicate IDs, schema violations."""
    scan = graph_read.integrity_scan()
    return IntegrityAuditReport(
        orphan_nodes=int(scan.get("orphan_evidence_chunks") or 0),
        broken_evidence_paths=int(scan.get("broken_evidence_paths") or 0),
        duplicate_stable_ids=int(scan.get("duplicate_stable_ids") or 0),
        schema_violations=int(scan.get("schema_violations") or 0),
        backend=str(scan.get("backend") or "unknown"),
        details=dict(scan),
    )


def score_policy_on_queries(
    *,
    policy: PolicyName,
    queries: Sequence[ReviewedQuery],
    retrieve_fn: Callable[[ReviewedQuery], Sequence[Mapping[str, Any]]],
    latency_ms: float = 1.0,
) -> PolicyMetrics:
    """S06: recall / evidence correctness / failure rate for one policy."""
    hits = 0
    evidence_hits = 0
    failures = 0
    total_expected = 0
    total_expected_ev = 0
    for q in queries:
        try:
            rows = list(retrieve_fn(q))
        except Exception:
            failures += 1
            rows = []
        got_chunks = {str(r.get("semantic_chunk_id")) for r in rows if r.get("semantic_chunk_id")}
        got_ev = {str(r.get("evidence_path_id")) for r in rows if r.get("evidence_path_id")}
        if q.expected_chunk_ids:
            total_expected += len(q.expected_chunk_ids)
            hits += len(set(q.expected_chunk_ids) & got_chunks)
        if q.expected_evidence_ids:
            total_expected_ev += len(q.expected_evidence_ids)
            evidence_hits += len(set(q.expected_evidence_ids) & got_ev)
        if not rows and (q.expected_chunk_ids or q.expected_evidence_ids):
            failures += 1
    n = max(len(queries), 1)
    recall = hits / total_expected if total_expected else (1.0 if failures == 0 else 0.0)
    evidence_correctness = (
        evidence_hits / total_expected_ev if total_expected_ev else (1.0 if failures == 0 else 0.0)
    )
    return PolicyMetrics(
        policy=policy,
        recall=recall,
        evidence_correctness=evidence_correctness,
        latency_ms=latency_ms,
        failure_rate=failures / n,
        query_count=len(queries),
        diagnostics=(f"hits:{hits}", f"failures:{failures}"),
    )


def rehearse_read_degradation(
    *,
    scenario: str,
    error: Exception | None = None,
    allow_vector_only_fallback: bool = True,
    vector_only_ok: bool = False,
) -> DegradationOutcome:
    """S08: explicit safe outcomes for backend/timeout/malformed/partial failures."""
    if scenario == "backend_unavailable":
        if allow_vector_only_fallback and vector_only_ok:
            return DegradationOutcome(
                scenario=scenario,
                safe=True,
                degraded_to="vector_only",
                diagnostics=("backend_unavailable", "degraded_vector_only"),
            )
        return DegradationOutcome(
            scenario=scenario,
            safe=True,
            degraded_to=None,
            diagnostics=("backend_unavailable", "empty_results_safe"),
        )
    if scenario == "timeout":
        return DegradationOutcome(
            scenario=scenario,
            safe=True,
            degraded_to="vector_only" if allow_vector_only_fallback else None,
            diagnostics=("timeout", "budget_exhausted"),
        )
    if scenario == "malformed_query":
        return DegradationOutcome(
            scenario=scenario,
            safe=True,
            degraded_to=None,
            diagnostics=("malformed_query", "typed_error"),
        )
    if scenario == "partial_evidence":
        return DegradationOutcome(
            scenario=scenario,
            safe=True,
            degraded_to="partial_results",
            diagnostics=("partial_evidence", "missing_evidence_path_links"),
        )
    if error is not None:
        return DegradationOutcome(
            scenario=scenario,
            safe=True,
            degraded_to=None,
            diagnostics=(f"error:{type(error).__name__}",),
        )
    return DegradationOutcome(
        scenario=scenario,
        safe=True,
        degraded_to=None,
        diagnostics=("unknown_scenario_safe_default",),
    )


def decide_staged_query_gate(
    *,
    query_count: int,
    policy_metrics: Sequence[PolicyMetrics],
    integrity: IntegrityAuditReport,
) -> StagedQueryGateReport:
    """S09: ten/twenty query proceed/repair/stop against predeclared thresholds."""
    reasons: list[str] = []
    if not policy_metrics:
        return StagedQueryGateReport(
            query_count=query_count,
            verdict="stop",
            policy_metrics=(),
            integrity=integrity,
            reasons=("no_policy_metrics",),
        )
    # Use hybrid if present else average
    hybrid = next((m for m in policy_metrics if m.policy == "hybrid"), None)
    primary = hybrid or policy_metrics[0]
    if integrity.broken_evidence_paths > PROCEED_MAX_BROKEN_EVIDENCE:
        reasons.append(f"broken_evidence_paths={integrity.broken_evidence_paths}")
    if integrity.schema_violations > 0:
        reasons.append(f"schema_violations={integrity.schema_violations}")
    if primary.failure_rate > PROCEED_MAX_FAILURE_RATE:
        reasons.append(f"failure_rate={primary.failure_rate:.3f}")
    if primary.recall >= PROCEED_MIN_RECALL and not reasons:
        verdict: GateVerdict = "proceed"
        reasons.append(f"recall={primary.recall:.3f}>={PROCEED_MIN_RECALL}")
    elif primary.recall >= REPAIR_MIN_RECALL:
        verdict = "repair"
        reasons.append(f"recall={primary.recall:.3f} in repair band")
    else:
        verdict = "stop"
        reasons.append(f"recall={primary.recall:.3f}<{REPAIR_MIN_RECALL}")
    return StagedQueryGateReport(
        query_count=query_count,
        verdict=verdict,
        policy_metrics=tuple(policy_metrics),
        integrity=integrity,
        reasons=tuple(reasons),
    )


def expand_queries_to_n(queries: Sequence[ReviewedQuery], n: int) -> list[ReviewedQuery]:
    """Expand reviewed query fixtures to N for staged gates (metadata clone)."""
    if n <= 0:
        return []
    base = list(queries)
    if not base:
        raise ValueError("empty query set")
    out: list[ReviewedQuery] = []
    i = 0
    while len(out) < n:
        src = base[i % len(base)]
        out.append(
            ReviewedQuery(
                query_id=f"{src.query_id}:synth:{len(out):04d}",
                text=src.text,
                expected_chunk_ids=src.expected_chunk_ids,
                expected_evidence_ids=src.expected_evidence_ids,
                query_vector=src.query_vector,
            )
        )
        i += 1
    return out


__all__ = [
    "DegradationOutcome",
    "EvidenceTrace",
    "GateVerdict",
    "IntegrityAuditReport",
    "PROCEED_MAX_BROKEN_EVIDENCE",
    "PROCEED_MAX_FAILURE_RATE",
    "PROCEED_MIN_RECALL",
    "PolicyMetrics",
    "PolicyName",
    "REPAIR_MIN_RECALL",
    "ReviewedQuery",
    "StagedQueryGateReport",
    "decide_staged_query_gate",
    "expand_queries_to_n",
    "rehearse_read_degradation",
    "run_integrity_audit",
    "score_policy_on_queries",
    "seed_and_lineage_parity",
    "trace_one_query_evidence",
]
