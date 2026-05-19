"""Read-only graph-readiness exporter for M004/S05 validation.

The exporter maps existing full-text ingestion, PageIndex, SemanticChunk, and
EvidencePath outputs into the graph-readiness contract.  It writes diagnostics
and summaries only; it never runs scientific extraction or persists KG data.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from arxiv_archive.evidence import build_semantic_chunks
from arxiv_archive.full_text import FullTextIngestionResult, FullTextSource, ingest_full_text
from arxiv_archive.graph_readiness import (
    ChunkRoute,
    ChunkType,
    CoordinateSpace,
    EvidencePathRef,
    ExtractionTrustLevel,
    GraphReadinessReport,
    GraphReadinessState,
    GraphReadyChunk,
    NormalizedPaperPackage,
    QualityWarning,
    SourceSpan,
    WarningSeverity,
    stable_text_hash,
    to_redacted_dict,
    validate_normalized_package,
)
from arxiv_archive.page_index import PageIndexDocument, PageIndexNode, build_page_index

CONTRACT_VERSION = "graph-ready-data-contract.v1"
SCHEMA_VERSION = "graph-readiness.schema.v1"
NORMALIZER_VERSION = "graph-readiness-export.v1"
EVENTS_FILENAME = "graph-readiness-events.jsonl"
SUMMARY_FILENAME = "graph-readiness-summary.json"
EXTRACTION_ROUTES = {
    ChunkRoute.CLAIM_EXTRACTION,
    ChunkRoute.METHOD_EXTRACTION,
    ChunkRoute.ENTITY_CANDIDATE_EXTRACTION,
    ChunkRoute.RELATION_EXTRACTION,
    ChunkRoute.TABLE_EXTRACTION,
    ChunkRoute.FIGURE_EVIDENCE,
    ChunkRoute.CITATION_GRAPH,
    ChunkRoute.METADATA_GRAPH,
}
CLAIM_EXTRACTION_TOKEN_LIMIT = 220
MIN_SPLIT_TOKEN_COUNT = 35
MAX_SPLIT_TOKEN_COUNT = 180
METADATA_SIGNAL_TERMS = (
    "affiliation",
    "correspondence",
    "orcid",
    "university",
    "institute",
    "technology co",
    "author information",
    "email",
)
ADMINISTRATIVE_ROADMAP_TERMS = (
    "paper is organized as follows",
    "paper is structured as follows",
    "remainder of this paper",
    "rest of this paper",
    "the rest of the paper",
    "in section ",
)
BACKGROUND_SECTION_TERMS = ("background", "related work", "prior work", "literature review")
BACKGROUND_BODY_TERMS = ("previous work", "prior studies", "has been studied", "existing work")
MULTI_CLAIM_SIGNAL_TERMS = (
    "our contributions are",
    "we make the following contributions",
    "main contributions",
    "we contribute",
)


@dataclass(frozen=True)
class ExportResult:
    """Result of exporting graph-readiness packages for a corpus."""

    packages: list[NormalizedPaperPackage]
    events_path: Path
    summary_path: Path
    summary: dict[str, Any]


@dataclass(frozen=True)
class SplitCandidate:
    """Source-span-preserving prose split candidate."""

    text: str
    source_span: SourceSpan
    order: int


def export_corpus(corpus_path: Path, output_dir: Path, *, run_id: str | None = None) -> ExportResult:
    """Export graph-readiness diagnostics for every document in a corpus manifest."""
    run_id = run_id or _default_run_id()
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    events_path = output_dir / EVENTS_FILENAME
    summary_path = output_dir / SUMMARY_FILENAME
    if events_path.exists():
        events_path.unlink()

    corpus = json.loads(Path(corpus_path).read_text(encoding="utf-8"))
    documents = corpus.get("documents", [])
    packages: list[NormalizedPaperPackage] = []
    for document in documents:
        package = build_package_from_manifest_document(document, run_id=run_id)
        packages.append(package)
        _append_event(events_path, _package_event(package))
        for route, details in package.report.routes.items():
            _append_event(
                events_path,
                {
                    "event": "graph_readiness.route",
                    "run_id": run_id,
                    "paper_id": package.paper_id,
                    "route": route,
                    **details,
                },
            )

    summary = _summarize_packages(run_id, packages)
    summary_path.write_text(json.dumps(to_redacted_dict(summary), indent=2, sort_keys=True), encoding="utf-8")
    return ExportResult(packages=packages, events_path=events_path, summary_path=summary_path, summary=summary)


def build_package_from_manifest_document(
    document: dict[str, Any],
    *,
    run_id: str,
    created_at: str | None = None,
) -> NormalizedPaperPackage:
    """Build a normalized graph-readiness package for one corpus manifest document."""
    paper_id = str(document["paper_id"])
    source_path = Path(
        document.get("expected_full_text_path")
        or Path(document.get("paper_dir", "")) / "full_text.md"
    )
    source = FullTextSource(paper_id=paper_id, source_type="markdown", source_path=source_path)
    ingestion = ingest_full_text(source)
    conversion_method = _read_conversion_method(source_path)
    conversion_id = f"{paper_id}:conversion:{conversion_method}:{stable_text_hash(str(source_path))[:12]}"
    document_id = f"{paper_id}:normalized:{stable_text_hash(ingestion.text)[:12]}"
    created_at = created_at or datetime.now(UTC).isoformat()

    conversion_warnings = _conversion_warnings(ingestion, conversion_method)
    if _blocks_document(ingestion, conversion_warnings):
        chunks: list[GraphReadyChunk] = []
        evidence_paths: list[EvidencePathRef] = []
        sections: list[dict[str, Any]] = []
    else:
        page_index = build_page_index(ingestion)
        chunks = _graph_ready_chunks(ingestion, page_index)
        evidence_paths = _evidence_path_refs(
            conversion_id=conversion_id,
            document_id=document_id,
            page_index=page_index,
            chunks=chunks,
        )
        sections = [_section_record(ingestion, node) for node in page_index.nodes]

    report = _report(
        run_id=run_id,
        paper_id=paper_id,
        conversion_id=conversion_id,
        document_id=document_id,
        chunks=chunks,
        evidence_paths=evidence_paths,
        warnings=conversion_warnings,
    )
    package = NormalizedPaperPackage(
        contract_version=CONTRACT_VERSION,
        run_id=run_id,
        created_at=created_at,
        schema_version=SCHEMA_VERSION,
        normalizer_version=NORMALIZER_VERSION,
        paper_id=paper_id,
        conversion_id=conversion_id,
        document_id=document_id,
        chunks=chunks,
        evidence_paths=evidence_paths,
        report=report,
        sections=sections,
        warnings=conversion_warnings,
    )
    validation = validate_normalized_package(package)
    if validation.warnings == package.warnings and validation.ok == _state_allows_package(report.state):
        return package

    merged_warnings = [*package.warnings, *validation.warnings]
    repaired_report = _report(
        run_id=run_id,
        paper_id=paper_id,
        conversion_id=conversion_id,
        document_id=document_id,
        chunks=chunks,
        evidence_paths=evidence_paths,
        warnings=merged_warnings,
    )
    return NormalizedPaperPackage(
        contract_version=package.contract_version,
        run_id=package.run_id,
        created_at=package.created_at,
        schema_version=package.schema_version,
        normalizer_version=package.normalizer_version,
        paper_id=package.paper_id,
        conversion_id=package.conversion_id,
        document_id=package.document_id,
        chunks=package.chunks,
        evidence_paths=package.evidence_paths,
        report=repaired_report,
        sections=package.sections,
        warnings=merged_warnings,
    )


def _graph_ready_chunks(
    ingestion: FullTextIngestionResult,
    page_index: PageIndexDocument,
) -> list[GraphReadyChunk]:
    semantic_chunks = build_semantic_chunks(page_index)
    chunks: list[GraphReadyChunk] = []
    for semantic_chunk in semantic_chunks:
        node = page_index.node_by_id(semantic_chunk.page_index_node_id)
        if node is None:
            continue
        source_span = _source_span_for_node(ingestion, node)
        token_count = len(semantic_chunk.text.split())
        chunk_type, routes, excluded_routes = _route_for_node(node)
        split_candidates = _split_prose_candidates(
            ingestion=ingestion,
            node=node,
            base_span=source_span,
            chunk_type=chunk_type,
            routes=routes,
            token_count=token_count,
        )
        if split_candidates:
            for split_candidate in split_candidates:
                split_chunk_id = f"{semantic_chunk.id}:split-{split_candidate.order:04d}"
                split_routes, split_excluded, split_warnings = _repair_route_quality(
                    chunk_id=split_chunk_id,
                    chunk_type=chunk_type,
                    routes=routes,
                    excluded_routes=excluded_routes,
                    token_count=len(split_candidate.text.split()),
                )
                candidate_warnings = _candidate_quality_warnings(
                    chunk_id=split_chunk_id,
                    node=node,
                    text=split_candidate.text,
                    routes=split_routes,
                )
                split_routes, split_excluded = _apply_candidate_route_classification(
                    routes=split_routes,
                    excluded_routes=split_excluded,
                    warnings=candidate_warnings,
                )
                chunks.append(
                    _graph_ready_chunk(
                        semantic_chunk_id=semantic_chunk.id,
                        split_suffix=f"split-{split_candidate.order:04d}",
                        paper_id=semantic_chunk.paper_id,
                        node=node,
                        order=(semantic_chunk.order * 1000) + split_candidate.order,
                        chunk_type=chunk_type,
                        routes=split_routes,
                        excluded_routes=split_excluded,
                        source_span=split_candidate.source_span,
                        text=split_candidate.text,
                        token_count=len(split_candidate.text.split()),
                        chunking_strategy=semantic_chunk.chunking_strategy,
                        quality_state=_chunk_quality_state(
                            traceable=split_candidate.source_span.is_graph_traceable(),
                            routes=split_routes,
                            route_warnings=[*split_warnings, *candidate_warnings],
                        ),
                        validation_warnings=[
                            QualityWarning(
                                code="prose_candidate_split",
                                severity=WarningSeverity.INFO,
                                message="Oversized prose chunk was split into a narrower source-span-preserving candidate.",
                                object_type="GraphReadyChunk",
                                object_id=f"{semantic_chunk.id}:split-{split_candidate.order:04d}",
                                route_impact=split_routes,
                                evidence={"parent_chunk_id": semantic_chunk.id},
                            ),
                            *split_warnings,
                            *candidate_warnings,
                        ],
                        provenance={**dict(semantic_chunk.provenance), "split_from_chunk_id": semantic_chunk.id},
                        parent_chunk_id=semantic_chunk.id,
                    )
                )
            continue
        routes, excluded_routes, route_warnings = _repair_route_quality(
            chunk_id=semantic_chunk.id,
            chunk_type=chunk_type,
            routes=routes,
            excluded_routes=excluded_routes,
            token_count=token_count,
        )
        candidate_warnings = _candidate_quality_warnings(
            chunk_id=semantic_chunk.id,
            node=node,
            text=semantic_chunk.text,
            routes=routes,
        )
        routes, excluded_routes = _apply_candidate_route_classification(
            routes=routes,
            excluded_routes=excluded_routes,
            warnings=candidate_warnings,
        )
        traceable = source_span.is_graph_traceable()
        quality_state = _chunk_quality_state(
            traceable=traceable,
            routes=routes,
            route_warnings=[*route_warnings, *candidate_warnings],
        )
        validation_warnings = [
            _warning_from_text(
                code="semantic_chunk_warning",
                message=warning,
                severity=WarningSeverity.WARN,
                object_type="GraphReadyChunk",
                object_id=semantic_chunk.id,
                route_impact=routes,
            )
            for warning in semantic_chunk.validation_warnings
        ]
        validation_warnings.extend([*route_warnings, *candidate_warnings])
        chunks.append(
            GraphReadyChunk(
                chunk_id=semantic_chunk.id,
                paper_id=semantic_chunk.paper_id,
                parent_element_ids=[node.id],
                page_index_node_id=node.id,
                section_path=list(node.path),
                order=semantic_chunk.order,
                chunk_type=chunk_type,
                routes=routes,
                excluded_routes=excluded_routes,
                source_span=source_span,
                text_hash=stable_text_hash(semantic_chunk.text),
                char_count=len(semantic_chunk.text),
                token_count=token_count,
                chunking_strategy=semantic_chunk.chunking_strategy,
                chunking_version="current_pageindex_semanticchunk_v1",
                quality_state=quality_state,
                validation_warnings=validation_warnings,
                provenance=dict(semantic_chunk.provenance),
            )
        )
    return chunks


def _graph_ready_chunk(
    *,
    semantic_chunk_id: str,
    split_suffix: str | None,
    paper_id: str,
    node: PageIndexNode,
    order: int,
    chunk_type: ChunkType,
    routes: list[ChunkRoute],
    excluded_routes: list[ChunkRoute],
    source_span: SourceSpan,
    text: str,
    token_count: int,
    chunking_strategy: str,
    quality_state: GraphReadinessState,
    validation_warnings: list[QualityWarning],
    provenance: dict[str, str],
    parent_chunk_id: str | None = None,
) -> GraphReadyChunk:
    chunk_id = semantic_chunk_id if split_suffix is None else f"{semantic_chunk_id}:{split_suffix}"
    return GraphReadyChunk(
        chunk_id=chunk_id,
        paper_id=paper_id,
        parent_element_ids=[node.id],
        page_index_node_id=node.id,
        section_path=list(node.path),
        order=order,
        chunk_type=chunk_type,
        routes=routes,
        excluded_routes=excluded_routes,
        source_span=source_span,
        text_hash=stable_text_hash(text),
        char_count=len(text),
        token_count=token_count,
        chunking_strategy=chunking_strategy,
        chunking_version="current_pageindex_semanticchunk_v2_split_prose",
        parent_chunk_id=parent_chunk_id,
        quality_state=quality_state,
        validation_warnings=validation_warnings,
        provenance=provenance,
    )


def _split_prose_candidates(
    *,
    ingestion: FullTextIngestionResult,
    node: PageIndexNode,
    base_span: SourceSpan,
    chunk_type: ChunkType,
    routes: list[ChunkRoute],
    token_count: int,
) -> list[SplitCandidate]:
    if ChunkRoute.CLAIM_EXTRACTION not in routes and ChunkRoute.METHOD_EXTRACTION not in routes:
        return []
    if chunk_type in {ChunkType.METADATA, ChunkType.REFERENCE_ENTRY, ChunkType.TABLE_CONTEXT, ChunkType.FIGURE_CAPTION_CONTEXT}:
        return []
    if token_count <= CLAIM_EXTRACTION_TOKEN_LIMIT:
        return []
    if not base_span.is_graph_traceable() or base_span.char_start is None:
        return []

    paragraphs = [paragraph.strip() for paragraph in node.text.split("\n\n") if paragraph.strip()]
    candidates: list[SplitCandidate] = []
    search_start = base_span.char_start
    for paragraph in paragraphs:
        paragraph_token_count = len(paragraph.split())
        if paragraph_token_count < MIN_SPLIT_TOKEN_COUNT or paragraph_token_count > MAX_SPLIT_TOKEN_COUNT:
            continue
        char_start = ingestion.text.find(paragraph, search_start)
        if char_start < 0:
            char_start = ingestion.text.find(paragraph)
        if char_start < 0:
            continue
        char_end = char_start + len(paragraph)
        candidates.append(
            SplitCandidate(
                text=paragraph,
                source_span=SourceSpan(
                    source_path=str(ingestion.source_path),
                    coordinate_space=CoordinateSpace.NORMALIZED_MARKDOWN_CHAR,
                    char_start=char_start,
                    char_end=char_end,
                    span_confidence=1.0,
                ),
                order=len(candidates) + 1,
            )
        )
        search_start = char_end
    if len(candidates) <= 1:
        return []
    return candidates


def _evidence_path_refs(
    *,
    conversion_id: str,
    document_id: str,
    page_index: PageIndexDocument,
    chunks: list[GraphReadyChunk],
) -> list[EvidencePathRef]:
    refs: list[EvidencePathRef] = []
    for graph_chunk in chunks:
        refs.append(
            EvidencePathRef(
                evidence_path_id=f"{graph_chunk.paper_id}:evidence:{graph_chunk.chunk_id}",
                paper_id=graph_chunk.paper_id,
                conversion_id=conversion_id,
                document_id=document_id,
                source_element_ids=list(graph_chunk.parent_element_ids),
                chunk_id=graph_chunk.chunk_id,
                section_path=list(graph_chunk.section_path),
                source_spans=[graph_chunk.source_span],
                route=graph_chunk.routes[0],
                quality_state=graph_chunk.quality_state,
                validation_warnings=list(graph_chunk.validation_warnings),
                provenance={
                    **dict(graph_chunk.provenance),
                    "page_index_node_id": graph_chunk.page_index_node_id or "unknown",
                    "evidence_builder": "graph_readiness_export_v2",
                    "page_index_node_count": str(len(page_index.nodes)),
                },
            )
        )
    return refs


def _section_record(ingestion: FullTextIngestionResult, node: PageIndexNode) -> dict[str, Any]:
    source_span = _source_span_for_node(ingestion, node)
    return {
        "section_id": node.id,
        "paper_id": node.paper_id,
        "title_hash": stable_text_hash(node.title),
        "level": node.level,
        "order": node.order,
        "parent_id": node.parent_id,
        "children_ids": list(node.children_ids),
        "path": list(node.path),
        "source_span": source_span,
    }


def _source_span_for_node(ingestion: FullTextIngestionResult, node: PageIndexNode) -> SourceSpan:
    stripped = node.text.strip()
    if not stripped:
        return SourceSpan(
            source_path=str(ingestion.source_path),
            coordinate_space=CoordinateSpace.NORMALIZED_MARKDOWN_CHAR,
            char_start=None,
            char_end=None,
            span_confidence=0.0,
        )
    char_start = ingestion.text.find(stripped)
    if char_start < 0:
        return SourceSpan(
            source_path=str(ingestion.source_path),
            coordinate_space=CoordinateSpace.ELEMENT_LOCAL_CHAR,
            char_start=0,
            char_end=len(stripped),
            span_confidence=0.5,
        )
    return SourceSpan(
        source_path=str(ingestion.source_path),
        coordinate_space=CoordinateSpace.NORMALIZED_MARKDOWN_CHAR,
        char_start=char_start,
        char_end=char_start + len(stripped),
        span_confidence=1.0,
    )


def _route_for_node(node: PageIndexNode) -> tuple[ChunkType, list[ChunkRoute], list[ChunkRoute]]:
    title = node.title.casefold()
    body = node.text.casefold()
    combined = f"{title}\n{body}"
    if "reference" in title:
        return (
            ChunkType.REFERENCE_ENTRY,
            [ChunkRoute.RETRIEVAL_ONLY],
            [ChunkRoute.CITATION_GRAPH, ChunkRoute.CLAIM_EXTRACTION, ChunkRoute.RELATION_EXTRACTION],
        )
    if any(term in combined for term in METADATA_SIGNAL_TERMS) or any(
        term in title for term in ("competing", "availability", "acknowledg", "author")
    ):
        return (
            ChunkType.METADATA,
            [ChunkRoute.METADATA_GRAPH, ChunkRoute.RETRIEVAL_ONLY],
            [ChunkRoute.CLAIM_EXTRACTION, ChunkRoute.RELATION_EXTRACTION],
        )
    if "table" in title:
        return (
            ChunkType.TABLE_CONTEXT,
            [ChunkRoute.RETRIEVAL_ONLY],
            [ChunkRoute.TABLE_EXTRACTION, ChunkRoute.CLAIM_EXTRACTION],
        )
    if "figure" in title:
        return (
            ChunkType.FIGURE_CAPTION_CONTEXT,
            [ChunkRoute.FIGURE_EVIDENCE, ChunkRoute.RETRIEVAL_ONLY],
            [],
        )
    if "method" in title:
        return (
            ChunkType.METHOD_CANDIDATE,
            [ChunkRoute.METHOD_EXTRACTION, ChunkRoute.ENTITY_CANDIDATE_EXTRACTION],
            [],
        )
    if "result" in title:
        return (
            ChunkType.RESULT_CANDIDATE,
            [ChunkRoute.CLAIM_EXTRACTION, ChunkRoute.RELATION_EXTRACTION],
            [],
        )
    return (
        ChunkType.CLAIM_CANDIDATE,
        [ChunkRoute.CLAIM_EXTRACTION, ChunkRoute.RETRIEVAL_ONLY],
        [],
    )


def _repair_route_quality(
    *,
    chunk_id: str,
    chunk_type: ChunkType,
    routes: list[ChunkRoute],
    excluded_routes: list[ChunkRoute],
    token_count: int,
) -> tuple[list[ChunkRoute], list[ChunkRoute], list[QualityWarning]]:
    warnings: list[QualityWarning] = []
    repaired_routes = list(routes)
    repaired_excluded = list(excluded_routes)
    if ChunkRoute.CLAIM_EXTRACTION in repaired_routes and token_count > CLAIM_EXTRACTION_TOKEN_LIMIT:
        repaired_routes = [
            route
            for route in repaired_routes
            if route not in {ChunkRoute.CLAIM_EXTRACTION, ChunkRoute.RELATION_EXTRACTION}
        ]
        if ChunkRoute.RETRIEVAL_ONLY not in repaired_routes:
            repaired_routes.append(ChunkRoute.RETRIEVAL_ONLY)
        repaired_excluded = _dedupe_routes(
            [*repaired_excluded, ChunkRoute.CLAIM_EXTRACTION, ChunkRoute.RELATION_EXTRACTION]
        )
        warnings.append(
            QualityWarning(
                code="oversized_claim_chunk_retrieval_only",
                severity=WarningSeverity.REPAIR_REQUIRED,
                message=(
                    "Chunk exceeds claim extraction token limit and is routed to retrieval-only until finer "
                    "claim splitting exists."
                ),
                object_type="GraphReadyChunk",
                object_id=chunk_id,
                route_impact=[ChunkRoute.CLAIM_EXTRACTION, ChunkRoute.RETRIEVAL_ONLY],
                repair_hint="Split section-level prose into narrower paragraph or claim candidates before extraction.",
                evidence={"token_count": token_count, "token_limit": CLAIM_EXTRACTION_TOKEN_LIMIT},
            )
        )
    if chunk_type == ChunkType.REFERENCE_ENTRY:
        warnings.append(
            QualityWarning(
                code="reference_entry_split_required",
                severity=WarningSeverity.WARN,
                message="Reference chunks remain retrieval-only until individual bibliography entries are parsed.",
                object_type="GraphReadyChunk",
                object_id=chunk_id,
                route_impact=[ChunkRoute.CITATION_GRAPH, ChunkRoute.RETRIEVAL_ONLY],
                repair_hint="Split bibliography into individual reference records before citation graph extraction.",
            )
        )
    if chunk_type == ChunkType.TABLE_CONTEXT:
        warnings.append(
            QualityWarning(
                code="table_lineage_required",
                severity=WarningSeverity.WARN,
                message="Table context is retrieval-only until row, column, and caption lineage is proven.",
                object_type="GraphReadyChunk",
                object_id=chunk_id,
                route_impact=[ChunkRoute.TABLE_EXTRACTION, ChunkRoute.RETRIEVAL_ONLY],
                repair_hint="Preserve table row/column/caption structure before table extraction.",
            )
        )
    if chunk_type == ChunkType.FIGURE_CAPTION_CONTEXT:
        warnings.append(
            QualityWarning(
                code="figure_caption_only_evidence",
                severity=WarningSeverity.WARN,
                message="Figure evidence is caption-only; no image-derived evidence is proven.",
                object_type="GraphReadyChunk",
                object_id=chunk_id,
                route_impact=[ChunkRoute.FIGURE_EVIDENCE],
                repair_hint="Do not infer image-derived facts unless visual evidence is extracted and verified.",
            )
        )
    return _dedupe_routes(repaired_routes), _dedupe_routes(repaired_excluded), warnings


def _candidate_quality_warnings(
    *,
    chunk_id: str,
    node: PageIndexNode,
    text: str,
    routes: list[ChunkRoute],
) -> list[QualityWarning]:
    """Attach deterministic candidate-level scientific KG warnings without deleting text."""
    if ChunkRoute.CLAIM_EXTRACTION not in routes:
        return []

    warnings: list[QualityWarning] = []
    title = node.title.casefold()
    body = text.casefold()
    if _is_administrative_roadmap(body):
        warnings.append(
            QualityWarning(
                code="administrative_roadmap_not_claim_evidence",
                severity=WarningSeverity.REPAIR_REQUIRED,
                message="Candidate describes paper organization and must not be promoted as a scientific claim.",
                object_type="GraphReadyChunk",
                object_id=chunk_id,
                route_impact=[ChunkRoute.CLAIM_EXTRACTION, ChunkRoute.RETRIEVAL_ONLY],
                repair_hint="Exclude this candidate from claim extraction while preserving it for retrieval context.",
            )
        )
    if _is_background_or_related_work(title=title, body=body):
        warnings.append(
            QualityWarning(
                code="background_related_work_claim_caveat",
                severity=WarningSeverity.WARN,
                message="Candidate appears to summarize background or related work; do not treat it as an author claim without review.",
                object_type="GraphReadyChunk",
                object_id=chunk_id,
                route_impact=[ChunkRoute.CLAIM_EXTRACTION],
                repair_hint="Require reviewer confirmation that the claim is made by the paper, not by cited prior work.",
            )
        )
    if _is_multi_claim_bundle(body):
        warnings.append(
            QualityWarning(
                code="multi_claim_candidate_requires_atomic_split",
                severity=WarningSeverity.REPAIR_REQUIRED,
                message="Candidate appears to bundle multiple contribution/result claims and needs atomic decomposition.",
                object_type="GraphReadyChunk",
                object_id=chunk_id,
                route_impact=[ChunkRoute.CLAIM_EXTRACTION, ChunkRoute.RELATION_EXTRACTION],
                repair_hint="Split into one source-spanned claim candidate per contribution or result before extraction.",
            )
        )
    return warnings


def _apply_candidate_route_classification(
    *,
    routes: list[ChunkRoute],
    excluded_routes: list[ChunkRoute],
    warnings: list[QualityWarning],
) -> tuple[list[ChunkRoute], list[ChunkRoute]]:
    """Apply candidate-level exclusions while keeping retrieval evidence available."""
    if not any(warning.code == "administrative_roadmap_not_claim_evidence" for warning in warnings):
        return routes, excluded_routes
    repaired_routes = [
        route
        for route in routes
        if route not in {ChunkRoute.CLAIM_EXTRACTION, ChunkRoute.RELATION_EXTRACTION}
    ]
    if ChunkRoute.RETRIEVAL_ONLY not in repaired_routes:
        repaired_routes.append(ChunkRoute.RETRIEVAL_ONLY)
    repaired_excluded = _dedupe_routes([*excluded_routes, ChunkRoute.CLAIM_EXTRACTION, ChunkRoute.RELATION_EXTRACTION])
    return _dedupe_routes(repaired_routes), repaired_excluded


def _is_administrative_roadmap(body: str) -> bool:
    return any(term in body for term in ADMINISTRATIVE_ROADMAP_TERMS)


def _is_background_or_related_work(*, title: str, body: str) -> bool:
    if any(term in title for term in BACKGROUND_SECTION_TERMS):
        return True
    citation_markers = body.count(" et al") + body.count("[") + body.count("(")
    return any(term in body for term in BACKGROUND_BODY_TERMS) and citation_markers >= 2


def _is_multi_claim_bundle(body: str) -> bool:
    if any(term in body for term in MULTI_CLAIM_SIGNAL_TERMS):
        return True
    bullet_markers = body.count("\n-") + body.count("\n*") + body.count("; ")
    result_verbs = sum(body.count(term) for term in ("we show", "we prove", "we introduce", "we propose", "we demonstrate"))
    return bullet_markers >= 2 or result_verbs >= 2


def _chunk_quality_state(
    *,
    traceable: bool,
    routes: list[ChunkRoute],
    route_warnings: list[QualityWarning],
) -> GraphReadinessState:
    if not traceable:
        return GraphReadinessState.REPAIR_REQUIRED
    if any(warning.severity == WarningSeverity.REPAIR_REQUIRED for warning in route_warnings):
        return GraphReadinessState.OK_FOR_RETRIEVAL_ONLY if routes == [ChunkRoute.RETRIEVAL_ONLY] else GraphReadinessState.REPAIR_REQUIRED
    if routes == [ChunkRoute.RETRIEVAL_ONLY]:
        return GraphReadinessState.OK_FOR_RETRIEVAL_ONLY
    return GraphReadinessState.OK_FOR_GRAPH


def _dedupe_routes(routes: list[ChunkRoute]) -> list[ChunkRoute]:
    deduped: list[ChunkRoute] = []
    for route in routes:
        if route not in deduped:
            deduped.append(route)
    return deduped


def _conversion_warnings(
    ingestion: FullTextIngestionResult,
    conversion_method: str,
) -> list[QualityWarning]:
    warnings = [
        _warning_from_text(
            code=_warning_code_for_ingestion(ingestion),
            message=warning,
            severity=_severity_for_ingestion(ingestion),
            object_type="ConversionRecord",
            object_id=ingestion.paper_id,
        )
        for warning in ingestion.warnings
    ]
    if conversion_method == "pymupdf":
        warnings.append(
            QualityWarning(
                code="deprecated_pymupdf_cache",
                severity=WarningSeverity.REPAIR_REQUIRED,
                message="Deprecated PyMuPDF cache output cannot be accepted as quality fallback evidence.",
                object_type="ConversionRecord",
                object_id=ingestion.paper_id,
                repair_hint="Regenerate with Docling or another approved converter.",
            )
        )
    return warnings


def _warning_code_for_ingestion(ingestion: FullTextIngestionResult) -> str:
    if ingestion.extraction_mode == "missing_source":
        return "missing_source"
    if ingestion.extraction_mode == "empty_source":
        return "empty_source"
    if ingestion.extraction_mode == "low_quality_source":
        return "no_substantive_body"
    return "ingestion_warning"


def _severity_for_ingestion(ingestion: FullTextIngestionResult) -> WarningSeverity:
    if ingestion.extraction_mode in {"missing_source", "empty_source", "low_quality_source"}:
        return WarningSeverity.BLOCKER
    return WarningSeverity.WARN


def _blocks_document(
    ingestion: FullTextIngestionResult,
    conversion_warnings: list[QualityWarning],
) -> bool:
    return ingestion.extraction_mode in {"missing_source", "empty_source", "low_quality_source"} or any(
        warning.severity == WarningSeverity.BLOCKER for warning in conversion_warnings
    )


def _report(
    *,
    run_id: str,
    paper_id: str,
    conversion_id: str,
    document_id: str,
    chunks: list[GraphReadyChunk],
    evidence_paths: list[EvidencePathRef],
    warnings: list[QualityWarning],
) -> GraphReadinessReport:
    warnings_by_severity: dict[str, int] = {}
    for warning in warnings:
        warnings_by_severity[warning.severity.value] = warnings_by_severity.get(warning.severity.value, 0) + 1
    chunk_warnings = [warning for chunk in chunks for warning in chunk.validation_warnings]
    for warning in chunk_warnings:
        warnings_by_severity[warning.severity.value] = warnings_by_severity.get(warning.severity.value, 0) + 1
    blockers = [warning for warning in warnings if warning.severity == WarningSeverity.BLOCKER]
    repair_warnings = [warning for warning in warnings if warning.severity == WarningSeverity.REPAIR_REQUIRED]
    chunk_blockers = [chunk for chunk in chunks if chunk.quality_state == GraphReadinessState.REJECT]
    state = _aggregate_state(blockers=blockers, repairs=repair_warnings, chunks=chunks)
    trust_level = _trust_level_for_state(state)
    route_counts: dict[str, dict[str, int | str]] = {}
    for chunk in chunks:
        for route in chunk.routes:
            route_entry = route_counts.setdefault(route.value, {"eligible": 0, "blocked": 0})
            if chunk.quality_state == GraphReadinessState.OK_FOR_GRAPH and route in EXTRACTION_ROUTES:
                route_entry["eligible"] = int(route_entry["eligible"]) + 1
            elif route == ChunkRoute.RETRIEVAL_ONLY and chunk.quality_state in {
                GraphReadinessState.OK_FOR_GRAPH,
                GraphReadinessState.OK_FOR_RETRIEVAL_ONLY,
            }:
                route_entry["eligible"] = int(route_entry["eligible"]) + 1
            else:
                route_entry["blocked"] = int(route_entry["blocked"]) + 1
    graph_traceable = sum(1 for chunk in chunks if chunk.source_span.is_graph_traceable())
    return GraphReadinessReport(
        run_id=run_id,
        paper_id=paper_id,
        conversion_id=conversion_id,
        document_id=document_id,
        state=state,
        trust_level=trust_level,
        counts={
            "chunks": len(chunks),
            "evidence_paths": len(evidence_paths),
            "warnings": len(warnings) + len(chunk_warnings),
            "chunk_blockers": len(chunk_blockers),
        },
        coverage={
            "chunks_with_char_span_rate": graph_traceable / len(chunks) if chunks else 0.0,
            "evidence_path_rate": len(evidence_paths) / len(chunks) if chunks else 0.0,
        },
        routes=route_counts,
        warnings_by_severity=warnings_by_severity,
        blockers=blockers,
        repair_hints=[warning.repair_hint for warning in warnings if warning.repair_hint],
    )


def _aggregate_state(
    *,
    blockers: list[QualityWarning],
    repairs: list[QualityWarning],
    chunks: list[GraphReadyChunk],
) -> GraphReadinessState:
    if blockers:
        return GraphReadinessState.REJECT
    if repairs or any(chunk.quality_state == GraphReadinessState.REPAIR_REQUIRED for chunk in chunks):
        return GraphReadinessState.REPAIR_REQUIRED
    if not chunks:
        return GraphReadinessState.OK_FOR_RETRIEVAL_ONLY
    if all(chunk.routes == [ChunkRoute.RETRIEVAL_ONLY] for chunk in chunks):
        return GraphReadinessState.OK_FOR_RETRIEVAL_ONLY
    return GraphReadinessState.OK_FOR_GRAPH


def _trust_level_for_state(state: GraphReadinessState) -> ExtractionTrustLevel:
    if state == GraphReadinessState.OK_FOR_GRAPH:
        return ExtractionTrustLevel.TRUSTED_GRAPH
    if state == GraphReadinessState.OK_FOR_RETRIEVAL_ONLY:
        return ExtractionTrustLevel.RETRIEVAL_ONLY
    return ExtractionTrustLevel.BLOCKED


def _state_allows_package(state: GraphReadinessState) -> bool:
    return state in {GraphReadinessState.OK_FOR_GRAPH, GraphReadinessState.OK_FOR_RETRIEVAL_ONLY}


def _warning_from_text(
    *,
    code: str,
    message: str,
    severity: WarningSeverity,
    object_type: str,
    object_id: str,
    route_impact: list[ChunkRoute] | None = None,
    evidence: dict[str, Any] | None = None,
) -> QualityWarning:
    return QualityWarning(
        code=code,
        severity=severity,
        message=message,
        object_type=object_type,
        object_id=object_id,
        route_impact=route_impact or [],
        evidence=evidence or {},
    )


def _read_conversion_method(source_path: Path) -> str:
    candidates = [
        source_path.with_suffix(".method"),
        source_path.parent / f"{source_path.stem}.method",
        source_path.parent / "conversion.method",
    ]
    for candidate in candidates:
        if candidate.exists():
            return candidate.read_text(encoding="utf-8").strip() or "unknown"
    return "unknown"


def _package_event(package: NormalizedPaperPackage) -> dict[str, Any]:
    return {
        "event": "graph_readiness.paper",
        "run_id": package.run_id,
        "paper_id": package.paper_id,
        "conversion_id": package.conversion_id,
        "document_id": package.document_id,
        "state": package.report.state,
        "trust_level": package.report.trust_level,
        "counts": package.report.counts,
        "coverage": package.report.coverage,
        "warnings_by_severity": package.report.warnings_by_severity,
        "blocker_codes": [warning.code for warning in package.report.blockers],
    }


def _summarize_packages(run_id: str, packages: list[NormalizedPaperPackage]) -> dict[str, Any]:
    by_state: dict[str, int] = {}
    route_totals: dict[str, dict[str, int]] = {}
    for package in packages:
        by_state[package.report.state.value] = by_state.get(package.report.state.value, 0) + 1
        for route, details in package.report.routes.items():
            route_entry = route_totals.setdefault(route, {"eligible": 0, "blocked": 0})
            route_entry["eligible"] += int(details.get("eligible", 0))
            route_entry["blocked"] += int(details.get("blocked", 0))
    return {
        "run_id": run_id,
        "paper_count": len(packages),
        "states": by_state,
        "routes": route_totals,
        "papers": [
            {
                "paper_id": package.paper_id,
                "state": package.report.state,
                "trust_level": package.report.trust_level,
                "counts": package.report.counts,
                "warnings_by_severity": package.report.warnings_by_severity,
            }
            for package in packages
        ],
    }


def _append_event(path: Path, payload: dict[str, Any]) -> None:
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(to_redacted_dict(payload), sort_keys=True) + "\n")


def _default_run_id() -> str:
    return f"graph-readiness-{datetime.now(UTC).strftime('%Y%m%dT%H%M%SZ')}"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Export read-only graph-readiness diagnostics.")
    parser.add_argument("--corpus", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--run-id", default=None)
    parser.add_argument(
        "--no-persist",
        action="store_true",
        help="Accepted for explicitness; this command never persists KG data.",
    )
    args = parser.parse_args(argv)
    result = export_corpus(args.corpus, args.output_dir, run_id=args.run_id)
    sys.stdout.write(json.dumps(to_redacted_dict(result.summary), indent=2, sort_keys=True))
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
