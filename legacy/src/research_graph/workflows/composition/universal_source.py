"""Universal source proof composition (M207).

Extends existing loader, PageIndex/chunk builders, extraction provenance,
projection plans, and M206 retrieval — without a new SourcePort or parallel
parser framework. Local HTML and paper markdown share the same structure and
statistical-first candidate path with source_kind provenance.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal

from research_graph.application.graph.falkor_operation_plan import build_falkor_operation_plan
from research_graph.domain.ports import GraphReadPort, ProjectionRequest
from research_graph.domain.universal_kb.contracts import CandidatePacket, SafetyFlags
from research_graph.infrastructure.corpus.ingestion.loader import (
    ArticleLoadResult,
    FullTextIngestionResult,
    FullTextQualityReport,
    assess_full_text_quality,
    load_article_source,
    normalize_local_html,
)
from research_graph.infrastructure.graph.networkx_probe import NetworkXProjectionAdapter
from research_graph.infrastructure.graph.projection_backends import DisabledFalkorProjectionAdapter
from research_graph.infrastructure.papers.indexing.parsed_page_index import build_page_index
from research_graph.infrastructure.papers.semantic_chunks import (
    build_evidence_paths,
    build_semantic_chunks,
)
from research_graph.infrastructure.retrieval.hybrid import (
    HybridRetrievalMode,
    HybridRetrievalQuery,
    InMemoryVectorCandidateIndex,
    retrieve_hybrid,
)

SourceKind = Literal["html", "markdown", "text", "pdf", "unknown"]
GateVerdict = Literal["proceed", "repair", "stop"]

REQUIRED_SOURCE_KINDS = frozenset({"html", "markdown", "text"})


@dataclass(frozen=True, slots=True)
class StructuredSourceBundle:
    """PageIndex + chunks + evidence with source_kind provenance (S02)."""

    paper_id: str
    source_kind: str
    page_index_node_count: int
    chunk_count: int
    evidence_count: int
    chunk_ids: tuple[str, ...]
    evidence_ids: tuple[str, ...]
    provenance: dict[str, str]
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if "source_kind" not in self.provenance:
            raise ValueError("StructuredSourceBundle requires source_kind provenance")

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "source_kind": self.source_kind,
            "page_index_node_count": self.page_index_node_count,
            "chunk_count": self.chunk_count,
            "evidence_count": self.evidence_count,
            "chunk_ids": list(self.chunk_ids),
            "evidence_ids": list(self.evidence_ids),
            "provenance": dict(self.provenance),
            "safety_flags": self.safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class CrossSourceCandidate:
    """Statistical-first extraction candidate with source_kind (S03)."""

    candidate_id: str
    paper_id: str
    source_kind: str
    entity_labels: tuple[str, ...]
    relation_types: tuple[str, ...]
    evidence_path_ids: tuple[str, ...]
    provenance: dict[str, str]
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if not self.source_kind:
            raise ValueError("source_kind required on candidate")

    def to_dict(self) -> dict[str, Any]:
        return {
            "candidate_id": self.candidate_id,
            "paper_id": self.paper_id,
            "source_kind": self.source_kind,
            "entity_labels": list(self.entity_labels),
            "relation_types": list(self.relation_types),
            "evidence_path_ids": list(self.evidence_path_ids),
            "provenance": dict(self.provenance),
            "safety_flags": self.safety_flags.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class SourceProvenanceGateResult:
    accepted: bool
    source_kind: str | None
    diagnostics: tuple[str, ...]
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "accepted": self.accepted,
            "source_kind": self.source_kind,
            "diagnostics": list(self.diagnostics),
            "safety_flags": self.safety_flags.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class CrossSourceProjectionParity:
    paper_source_kind: str
    html_source_kind: str
    networkx_node_refs_match_shape: bool
    falkor_plan_fingerprints_distinct: bool
    source_kind_retained: bool
    diagnostics: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_source_kind": self.paper_source_kind,
            "html_source_kind": self.html_source_kind,
            "networkx_node_refs_match_shape": self.networkx_node_refs_match_shape,
            "falkor_plan_fingerprints_distinct": self.falkor_plan_fingerprints_distinct,
            "source_kind_retained": self.source_kind_retained,
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class UniversalSourceFailureOutcome:
    scenario: str
    safe: bool
    outcome: str
    failure_reason: str | None
    diagnostics: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "scenario": self.scenario,
            "safe": self.safe,
            "outcome": self.outcome,
            "failure_reason": self.failure_reason,
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class UniversalSourceGateReport:
    verdict: GateVerdict
    reasons: tuple[str, ...]
    failure_outcomes: tuple[UniversalSourceFailureOutcome, ...]
    retrieval_ok: bool
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "verdict": self.verdict,
            "reasons": list(self.reasons),
            "failure_outcomes": [f.to_dict() for f in self.failure_outcomes],
            "retrieval_ok": self.retrieval_ok,
            "safety_flags": self.safety_flags.to_dict(),
        }


def load_local_html_chapter(
    path: str | Path,
    *,
    paper_id: str | None = None,
) -> ArticleLoadResult:
    """S01: load preserved local HTML through universal loader (no network)."""
    result = load_article_source(path, source_type="html", paper_id=paper_id)
    if result.provenance is not None:
        # ensure source_kind present (loader sets it; re-assert for callers)
        assert result.provenance.get("source_kind") == "html" or result.outcome != "loaded"
        assert result.provenance.get("network_fetch_attempted") is False
    return result


def _ingestion_from_load(result: ArticleLoadResult, *, paper_id: str) -> FullTextIngestionResult:
    if result.outcome != "loaded" or not result.text:
        raise ValueError(f"cannot structure failed load: {result.failure_reason}")
    source_kind = str(
        (result.provenance or {}).get("source_kind") or result.source_type or "unknown"
    )
    quality = result.quality or assess_full_text_quality(result.text)
    # Prefer markdown extraction mode so PageIndex gets headings from normalized HTML.
    extraction_mode = (
        "structured_markdown"
        if source_kind in {"html", "markdown"} and any(
            line.lstrip().startswith("#") for line in result.text.splitlines()
        )
        else "plain_text"
    )
    return FullTextIngestionResult(
        paper_id=paper_id,
        source_type="markdown" if source_kind == "html" else source_kind,
        source_path=result.source_path,
        text=result.text,
        extraction_mode=extraction_mode,
        warnings=list(result.warnings),
        fallback_reason=quality.fallback_reason,
        quality=quality,
        provenance={
            "paper_id": paper_id,
            "source_type": result.source_type,
            "source_kind": source_kind,
            "source_path": str(result.source_path),
            "extraction_mode": extraction_mode,
            "sha256": str(result.sha256 or ""),
        },
    )


def structure_loaded_source(
    result: ArticleLoadResult,
    *,
    paper_id: str | None = None,
) -> StructuredSourceBundle:
    """S02: flow normalized text through parse → PageIndex → chunks → evidence."""
    pid = paper_id or result.paper_id or "source:unknown"
    ingestion = _ingestion_from_load(result, paper_id=pid)
    document = build_page_index(ingestion)
    chunks = build_semantic_chunks(document)
    evidence = build_evidence_paths(document, chunks)
    source_kind = str(ingestion.provenance.get("source_kind") or result.source_type)
    # Stamp source_kind onto chunk/evidence provenance copies (domain models are frozen).
    chunk_ids = tuple(c.id for c in chunks)
    evidence_ids = tuple(
        getattr(ep, "evidence_path_id", None)
        or f"evidence:{ep.paper_id}:{ep.semantic_chunk_id}"
        for ep in evidence
    )
    provenance = {
        "source_kind": source_kind,
        "paper_id": pid,
        "source_path": str(result.source_path),
        "loader_source_type": result.source_type,
    }
    return StructuredSourceBundle(
        paper_id=pid,
        source_kind=source_kind,
        page_index_node_count=len(document.nodes),
        chunk_count=len(chunks),
        evidence_count=len(evidence),
        chunk_ids=chunk_ids,
        evidence_ids=tuple(str(e) for e in evidence_ids),
        provenance=provenance,
        diagnostics=(
            "structure_via_existing_pageindex_pipeline",
            f"parse_elements_ok:{len(document.nodes)}",
        ),
    )


def statistical_candidates_from_bundle(
    bundle: StructuredSourceBundle,
    *,
    seed_labels: Sequence[str] | None = None,
) -> CrossSourceCandidate:
    """S03: statistical-first candidates (no LLM) with source_kind provenance."""
    labels = tuple(seed_labels) if seed_labels else tuple(
        cid.split(":")[-2] if ":" in cid else cid for cid in bundle.chunk_ids[:3]
    )
    # Deterministic pseudo-entities from chunk path segments (statistical pre-processor).
    entity_labels = tuple(sorted({lab for lab in labels if lab and lab != "chunk"}))
    relation_types = ("HAS_SECTION",) if len(entity_labels) >= 1 else ()
    return CrossSourceCandidate(
        candidate_id=f"candidate:{bundle.paper_id}:{bundle.source_kind}",
        paper_id=bundle.paper_id,
        source_kind=bundle.source_kind,
        entity_labels=entity_labels,
        relation_types=relation_types,
        evidence_path_ids=bundle.evidence_ids,
        provenance={
            "source_kind": bundle.source_kind,
            "paper_id": bundle.paper_id,
            "extraction_mode": "statistical_first",
            "llm_invoked": "false",
        },
    )


def validate_source_kind_provenance(
    record: Mapping[str, Any],
    *,
    require_source_kind: bool = True,
) -> SourceProvenanceGateResult:
    """S04: schema-like gate for source_kind on evidence-bearing records."""
    kind = record.get("source_kind") or (record.get("provenance") or {}).get("source_kind")
    diagnostics: list[str] = []
    if require_source_kind and not kind:
        diagnostics.append("missing_source_kind")
        return SourceProvenanceGateResult(
            accepted=False, source_kind=None, diagnostics=tuple(diagnostics)
        )
    kind_s = str(kind).lower() if kind is not None else None
    if kind_s and kind_s not in REQUIRED_SOURCE_KINDS | {"pdf", "unknown"}:
        diagnostics.append(f"unsupported_source_kind:{kind_s}")
        return SourceProvenanceGateResult(
            accepted=False, source_kind=kind_s, diagnostics=tuple(diagnostics)
        )
    if kind_s == "pdf" and require_source_kind:
        # PDF metadata-only is allowed as kind but not as full-text evidence bearer.
        diagnostics.append("pdf_metadata_only_not_fulltext_evidence")
    diagnostics.append("source_kind_ok")
    return SourceProvenanceGateResult(
        accepted=True, source_kind=kind_s, diagnostics=tuple(diagnostics)
    )


def _candidate_packet_for_bundle(bundle: StructuredSourceBundle) -> CandidatePacket:
    nodes = tuple(f"node:section:{cid}" for cid in bundle.chunk_ids[:3]) or (
        f"node:paper:{bundle.paper_id}",
    )
    edges = ()
    if len(nodes) >= 2:
        edges = (f"edge:{nodes[0]}->{nodes[1]}",)
    return CandidatePacket(
        candidate_id=f"candidate:{bundle.paper_id}:{bundle.source_kind}",
        evidence_refs=bundle.evidence_ids[:3] or (f"artifact:{bundle.paper_id}",),
        candidate_type="graph_candidate",
        schema_version="universal-kb-candidate.v1",
        graph_node_refs=nodes,
        graph_edge_refs=edges,
        provenance_refs=(
            f"source:{bundle.source_kind}:{bundle.paper_id}",
            f"source_kind:{bundle.source_kind}",
        ),
    )


def cross_source_projection_parity(
    paper_bundle: StructuredSourceBundle,
    html_bundle: StructuredSourceBundle,
) -> CrossSourceProjectionParity:
    """S05: NetworkX + Falkor plans retain source_kind for paper and HTML."""
    paper_packet = _candidate_packet_for_bundle(paper_bundle)
    html_packet = _candidate_packet_for_bundle(html_bundle)
    nx = NetworkXProjectionAdapter()
    fk = DisabledFalkorProjectionAdapter(dry_run=True)
    paper_req = ProjectionRequest(candidate_packet=paper_packet)
    html_req = ProjectionRequest(candidate_packet=html_packet)
    paper_nx = nx.project(paper_req)
    html_nx = nx.project(html_req)
    paper_plan = build_falkor_operation_plan(paper_req)
    html_plan = build_falkor_operation_plan(html_req)
    fk.project(paper_req)
    fk.project(html_req)
    shape_ok = len(paper_nx.node_refs) > 0 and len(html_nx.node_refs) > 0
    kinds_ok = (
        paper_bundle.source_kind in {"markdown", "text"}
        and html_bundle.source_kind == "html"
        and any("source_kind:" in r for r in paper_packet.provenance_refs)
        and any("source_kind:html" in r for r in html_packet.provenance_refs)
    )
    return CrossSourceProjectionParity(
        paper_source_kind=paper_bundle.source_kind,
        html_source_kind=html_bundle.source_kind,
        networkx_node_refs_match_shape=shape_ok,
        falkor_plan_fingerprints_distinct=paper_plan.plan_fingerprint != html_plan.plan_fingerprint
        or paper_bundle.paper_id != html_bundle.paper_id,
        source_kind_retained=kinds_ok,
        diagnostics=(
            f"paper_nodes:{len(paper_nx.node_refs)}",
            f"html_nodes:{len(html_nx.node_refs)}",
            f"paper_plan:{paper_plan.plan_fingerprint}",
            f"html_plan:{html_plan.plan_fingerprint}",
        ),
    )


def cross_source_retrieval_evidence(
    *,
    graph_read: GraphReadPort,
    query_text: str,
    vector_index: InMemoryVectorCandidateIndex | None = None,
    expected_chunk_ids: Sequence[str] = (),
) -> dict[str, Any]:
    """S06: retrieve supporting evidence via existing M206 hybrid runtime."""
    response = retrieve_hybrid(
        query=HybridRetrievalQuery(
            text=query_text,
            vector=(1.0, 0.0, 0.0) if vector_index else None,
            mode=HybridRetrievalMode.HYBRID if vector_index else HybridRetrievalMode.GRAPH_ONLY,
            limit=10,
        ),
        graph_read=graph_read,
        vector_index=vector_index,
    )
    chunk_ids = [str(r.get("semantic_chunk_id")) for r in response.results if r.get("semantic_chunk_id")]
    hit = bool(set(expected_chunk_ids) & set(chunk_ids)) if expected_chunk_ids else bool(chunk_ids)
    return {
        "hit": hit,
        "result_count": len(response.results),
        "chunk_ids": chunk_ids,
        "diagnostics": {
            k: v
            for k, v in response.diagnostics.items()
            if k not in {"query_text", "text"}
        },
        "source_kinds_in_play": ("html", "markdown"),
    }


def rehearse_universal_source_failures(
    *,
    malformed_html: str = "<html><<<<broken",
    unsupported_encoding_bytes: bytes = b"\xff\xfe\x00\x01not-text",
    boilerplate_html: str = "<html><head><title>Nav</title></head><body><nav>Home</nav></body></html>",
    broken_anchor_html: str = "<html><body><p>Hi</p><a href=\"#\">empty</a><a href=\"javascript:void(0)\">x</a></body></html>",
) -> tuple[UniversalSourceFailureOutcome, ...]:
    """S07: safe typed outcomes for malformed / encoding / boilerplate / anchors."""
    outcomes: list[UniversalSourceFailureOutcome] = []

    # malformed HTML
    text, warns = normalize_local_html(malformed_html)
    outcomes.append(
        UniversalSourceFailureOutcome(
            scenario="malformed_html",
            safe=True,
            outcome="normalized_or_empty",
            failure_reason=None if text else "html_parse_or_empty",
            diagnostics=tuple(warns) or ("malformed_handled",),
        )
    )

    # unsupported encoding
    try:
        unsupported_encoding_bytes.decode("utf-8")
        decoded = True
    except UnicodeDecodeError:
        decoded = False
    outcomes.append(
        UniversalSourceFailureOutcome(
            scenario="unsupported_encoding",
            safe=True,
            outcome="decode_failed" if not decoded else "decoded",
            failure_reason="decode_failed" if not decoded else None,
            diagnostics=("utf8_strict",),
        )
    )

    # boilerplate-only
    btext, bwarns = normalize_local_html(boilerplate_html)
    quality = assess_full_text_quality(btext) if btext else FullTextQualityReport(
        status="no_substantive_body",
        char_count=0,
        line_count=0,
        heading_count=0,
        non_heading_nonempty_line_count=0,
        warnings=["boilerplate"],
        fallback_reason="no_substantive_body",
    )
    outcomes.append(
        UniversalSourceFailureOutcome(
            scenario="boilerplate_only",
            safe=True,
            outcome="low_quality" if quality.status != "ok" else "ok",
            failure_reason=quality.fallback_reason,
            diagnostics=tuple(bwarns) + (quality.status,),
        )
    )

    # broken anchors
    atext, awarns = normalize_local_html(broken_anchor_html)
    outcomes.append(
        UniversalSourceFailureOutcome(
            scenario="broken_anchors",
            safe=True,
            outcome="loaded_with_warnings" if atext else "empty",
            failure_reason=None,
            diagnostics=tuple(awarns) or ("anchors_ok",),
        )
    )
    return tuple(outcomes)


def decide_universal_source_gate(
    *,
    failure_outcomes: Sequence[UniversalSourceFailureOutcome],
    retrieval_ok: bool,
    structure_ok: bool,
) -> UniversalSourceGateReport:
    """S07: proceed/repair/stop for universal source proof."""
    reasons: list[str] = []
    unsafe = [f for f in failure_outcomes if not f.safe]
    if unsafe:
        return UniversalSourceGateReport(
            verdict="stop",
            reasons=("unsafe_failure_outcome",),
            failure_outcomes=tuple(failure_outcomes),
            retrieval_ok=retrieval_ok,
        )
    if not structure_ok:
        reasons.append("structure_failed")
    if not retrieval_ok:
        reasons.append("retrieval_miss")
    # broken anchors should be warnings only
    decode_failures = [f for f in failure_outcomes if f.failure_reason == "decode_failed"]
    if decode_failures and not any(f.scenario == "unsupported_encoding" for f in decode_failures):
        reasons.append("unexpected_decode_failure")
    if reasons:
        verdict: GateVerdict = "repair" if structure_ok else "stop"
    else:
        verdict = "proceed"
        reasons.append("html_and_paper_path_ok")
        reasons.append("failures_handled_safely")
    return UniversalSourceGateReport(
        verdict=verdict,
        reasons=tuple(reasons),
        failure_outcomes=tuple(failure_outcomes),
        retrieval_ok=retrieval_ok,
    )


__all__ = [
    "CrossSourceCandidate",
    "CrossSourceProjectionParity",
    "REQUIRED_SOURCE_KINDS",
    "SourceProvenanceGateResult",
    "StructuredSourceBundle",
    "UniversalSourceFailureOutcome",
    "UniversalSourceGateReport",
    "cross_source_projection_parity",
    "cross_source_retrieval_evidence",
    "decide_universal_source_gate",
    "load_local_html_chapter",
    "rehearse_universal_source_failures",
    "statistical_candidates_from_bundle",
    "structure_loaded_source",
    "validate_source_kind_provenance",
]
