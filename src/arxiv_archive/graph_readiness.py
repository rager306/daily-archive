"""Graph-readiness contracts for route-gated scientific KG validation.

This module defines the data-shape and validation boundary introduced by
M004/S11.  It is intentionally persistence-free: callers can build
NormalizedPaperPackage records, validate them, and serialize redacted diagnostic
payloads before deciding whether extraction or graph persistence is allowed.
"""

from __future__ import annotations

import hashlib
from dataclasses import asdict, dataclass, field, is_dataclass
from enum import StrEnum
from pathlib import Path
from typing import Any


class GraphReadinessState(StrEnum):
    """Eligibility state for paper, chunk, route, or evidence objects."""

    OK_FOR_GRAPH = "ok_for_graph"
    OK_FOR_RETRIEVAL_ONLY = "ok_for_retrieval_only"
    REPAIR_REQUIRED = "repair_required"
    REJECT = "reject"


class WarningSeverity(StrEnum):
    """Severity for graph-readiness warnings."""

    INFO = "info"
    WARN = "warn"
    REPAIR_REQUIRED = "repair_required"
    BLOCKER = "blocker"


class ContentType(StrEnum):
    """Normalized source content categories."""

    PAPER_METADATA = "paper_metadata"
    ABSTRACT = "abstract"
    SECTION_HEADING = "section_heading"
    PARAGRAPH = "paragraph"
    TABLE = "table"
    TABLE_CAPTION = "table_caption"
    TABLE_ROW = "table_row"
    TABLE_CELL = "table_cell"
    FIGURE = "figure"
    FIGURE_CAPTION = "figure_caption"
    EQUATION = "equation"
    REFERENCE_ENTRY = "reference_entry"
    CITATION_MARKER = "citation_marker"
    AUTHOR_AFFILIATION = "author_affiliation"
    AVAILABILITY_STATEMENT = "availability_statement"
    ETHICS_STATEMENT = "ethics_statement"
    COMPETING_INTERESTS = "competing_interests"
    ACKNOWLEDGEMENTS = "acknowledgements"
    APPENDIX = "appendix"
    SUPPLEMENTARY = "supplementary"
    BOILERPLATE = "boilerplate"
    NAVIGATION_NOISE = "navigation_noise"
    UNKNOWN = "unknown"


class ChunkType(StrEnum):
    """Route-relevant chunk categories."""

    CLAIM_CANDIDATE = "claim_candidate"
    METHOD_CANDIDATE = "method_candidate"
    RESULT_CANDIDATE = "result_candidate"
    DEFINITION_CANDIDATE = "definition_candidate"
    TABLE_CONTEXT = "table_context"
    TABLE_ROW_GROUP = "table_row_group"
    FIGURE_CAPTION_CONTEXT = "figure_caption_context"
    EQUATION_CONTEXT = "equation_context"
    CITATION_CONTEXT = "citation_context"
    REFERENCE_ENTRY = "reference_entry"
    METADATA = "metadata"
    AVAILABILITY = "availability"
    ADMINISTRATIVE = "administrative"
    RETRIEVAL_CONTEXT = "retrieval_context"
    NOISE = "noise"
    UNKNOWN = "unknown"


class ChunkRoute(StrEnum):
    """Downstream routes a chunk may enter or be excluded from."""

    CLAIM_EXTRACTION = "claim_extraction"
    METHOD_EXTRACTION = "method_extraction"
    ENTITY_CANDIDATE_EXTRACTION = "entity_candidate_extraction"
    RELATION_EXTRACTION = "relation_extraction"
    TABLE_EXTRACTION = "table_extraction"
    FIGURE_EVIDENCE = "figure_evidence"
    CITATION_GRAPH = "citation_graph"
    METADATA_GRAPH = "metadata_graph"
    RETRIEVAL_ONLY = "retrieval_only"
    EXCLUDE_FROM_EXTRACTION = "exclude_from_extraction"
    REPAIR_QUEUE = "repair_queue"


class ExtractionTrustLevel(StrEnum):
    """Trust level derived from route and state validation."""

    TRUSTED_GRAPH = "trusted_graph"
    LOW_CONFIDENCE_GRAPH = "low_confidence_graph"
    RETRIEVAL_ONLY = "retrieval_only"
    BLOCKED = "blocked"


class CoordinateSpace(StrEnum):
    """Coordinate system used by a source span."""

    NORMALIZED_MARKDOWN_CHAR = "normalized_markdown_char"
    NORMALIZED_MARKDOWN_LINE = "normalized_markdown_line"
    PDF_PAGE_BBOX = "pdf_page_bbox"
    ELEMENT_LOCAL_CHAR = "element_local_char"


@dataclass(frozen=True)
class QualityWarning:
    """Machine-readable warning attached to a graph-readiness object."""

    code: str
    severity: WarningSeverity
    message: str
    object_type: str
    object_id: str
    route_impact: list[ChunkRoute] = field(default_factory=list)
    repair_hint: str | None = None
    evidence: dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class QualitySignal:
    """Metric-like signal used to support gate decisions."""

    name: str
    value: str | int | float | bool | None
    passed: bool
    threshold: str | int | float | None = None
    severity_on_fail: WarningSeverity = WarningSeverity.WARN
    details: dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class SourceSpan:
    """Traceable location in a converted or source artifact."""

    source_path: str
    coordinate_space: CoordinateSpace = CoordinateSpace.NORMALIZED_MARKDOWN_CHAR
    char_start: int | None = None
    char_end: int | None = None
    line_start: int | None = None
    line_end: int | None = None
    page_start: int | None = None
    page_end: int | None = None
    block_id: str | None = None
    bbox: dict[str, float] | None = None
    span_confidence: float = 1.0
    warnings: list[QualityWarning] = field(default_factory=list)

    def is_graph_traceable(self) -> bool:
        """Return whether this span is sufficient for trusted text evidence."""
        return (
            self.coordinate_space == CoordinateSpace.NORMALIZED_MARKDOWN_CHAR
            and self.char_start is not None
            and self.char_end is not None
            and self.char_start >= 0
            and self.char_end > self.char_start
            and self.span_confidence >= 0.8
        )


@dataclass(frozen=True)
class GraphReadyChunk:
    """Chunk contract used before scientific extraction."""

    chunk_id: str
    paper_id: str
    parent_element_ids: list[str]
    section_path: list[str]
    order: int
    chunk_type: ChunkType
    routes: list[ChunkRoute]
    source_span: SourceSpan
    text_hash: str
    char_count: int
    page_index_node_id: str | None = None
    excluded_routes: list[ChunkRoute] = field(default_factory=list)
    token_count: int | None = None
    chunking_strategy: str = "unknown"
    chunking_version: str = "unknown"
    parent_chunk_id: str | None = None
    child_chunk_ids: list[str] = field(default_factory=list)
    quality_state: GraphReadinessState = GraphReadinessState.OK_FOR_GRAPH
    quality_signals: list[QualitySignal] = field(default_factory=list)
    validation_warnings: list[QualityWarning] = field(default_factory=list)
    provenance: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class EvidencePathRef:
    """Normalized evidence-path reference for a graph-ready chunk."""

    evidence_path_id: str
    paper_id: str
    conversion_id: str
    document_id: str
    source_element_ids: list[str]
    chunk_id: str
    section_path: list[str]
    source_spans: list[SourceSpan]
    route: ChunkRoute
    quality_state: GraphReadinessState = GraphReadinessState.OK_FOR_GRAPH
    validation_warnings: list[QualityWarning] = field(default_factory=list)
    provenance: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class ChunkAnnotation:
    """Sidecar enrichment hints for a chunk.

    Annotation content is diagnostic and must not be promoted to KG facts without
    evidence-backed extraction.
    """

    annotation_id: str
    chunk_id: str
    paper_id: str
    method: str
    method_version: str | None = None
    keyphrases: list[dict[str, Any]] = field(default_factory=list)
    entity_candidates: list[dict[str, Any]] = field(default_factory=list)
    metric_candidates: list[dict[str, Any]] = field(default_factory=list)
    citation_markers: list[str] = field(default_factory=list)
    section_alignment: dict[str, Any] = field(default_factory=dict)
    salience_scores: dict[str, float] = field(default_factory=dict)
    quality_signals: list[QualitySignal] = field(default_factory=list)
    route_hints: list[ChunkRoute] = field(default_factory=list)
    warnings: list[QualityWarning] = field(default_factory=list)


@dataclass(frozen=True)
class GraphReadinessReport:
    """Per-paper or per-package graph-readiness summary."""

    run_id: str
    paper_id: str
    conversion_id: str
    document_id: str
    state: GraphReadinessState
    trust_level: ExtractionTrustLevel
    counts: dict[str, int] = field(default_factory=dict)
    coverage: dict[str, float] = field(default_factory=dict)
    routes: dict[str, dict[str, int | str]] = field(default_factory=dict)
    warnings_by_severity: dict[str, int] = field(default_factory=dict)
    blockers: list[QualityWarning] = field(default_factory=list)
    repair_hints: list[str] = field(default_factory=list)
    review_artifact_path: str | None = None
    machine_log_path: str | None = None


@dataclass(frozen=True)
class NormalizedPaperPackage:
    """Single graph-readiness unit emitted per paper per validation run."""

    contract_version: str
    run_id: str
    created_at: str
    schema_version: str
    normalizer_version: str
    paper_id: str
    conversion_id: str
    document_id: str
    chunks: list[GraphReadyChunk]
    evidence_paths: list[EvidencePathRef]
    report: GraphReadinessReport
    sections: list[dict[str, Any]] = field(default_factory=list)
    paragraphs: list[dict[str, Any]] = field(default_factory=list)
    tables: list[dict[str, Any]] = field(default_factory=list)
    figures: list[dict[str, Any]] = field(default_factory=list)
    equations: list[dict[str, Any]] = field(default_factory=list)
    references: list[dict[str, Any]] = field(default_factory=list)
    citation_markers: list[dict[str, Any]] = field(default_factory=list)
    citations: list[dict[str, Any]] = field(default_factory=list)
    annotations: list[ChunkAnnotation] = field(default_factory=list)
    warnings: list[QualityWarning] = field(default_factory=list)


@dataclass(frozen=True)
class ValidationResult:
    """Result of validating a normalized paper package."""

    ok: bool
    warnings: list[QualityWarning] = field(default_factory=list)

    @property
    def blockers(self) -> list[QualityWarning]:
        return [warning for warning in self.warnings if warning.severity == WarningSeverity.BLOCKER]


REDACTED_TEXT_KEYS = {
    "text",
    "markdown",
    "chunk_text",
    "raw_text",
    "prompt",
    "embedding",
    "embeddings",
}


def stable_text_hash(text: str) -> str:
    """Return a stable SHA-256 hash for text without exposing the text itself."""
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def validate_graph_ready_chunk(chunk: GraphReadyChunk) -> list[QualityWarning]:
    """Return contract violations for one graph-ready chunk."""
    warnings: list[QualityWarning] = []
    object_id = chunk.chunk_id

    if not chunk.routes:
        warnings.append(
            _blocker(
                "missing_chunk_route",
                "GraphReadyChunk must declare at least one downstream route.",
                "GraphReadyChunk",
                object_id,
            )
        )

    if not chunk.parent_element_ids:
        warnings.append(
            _repair(
                "missing_parent_element",
                "GraphReadyChunk must reference at least one parent normalized element.",
                "GraphReadyChunk",
                object_id,
            )
        )

    if not chunk.section_path:
        warnings.append(
            _repair(
                "missing_section_path",
                "GraphReadyChunk must preserve section lineage.",
                "GraphReadyChunk",
                object_id,
            )
        )

    if not chunk.source_span.is_graph_traceable():
        warnings.append(
            _blocker(
                "missing_or_untrusted_source_span",
                "GraphReadyChunk source span must use normalized Markdown char offsets with confidence >= 0.8.",
                "GraphReadyChunk",
                object_id,
            )
        )

    if chunk.quality_state in {GraphReadinessState.REPAIR_REQUIRED, GraphReadinessState.REJECT}:
        warnings.append(
            QualityWarning(
                code="chunk_state_blocks_extraction",
                severity=WarningSeverity.REPAIR_REQUIRED
                if chunk.quality_state == GraphReadinessState.REPAIR_REQUIRED
                else WarningSeverity.BLOCKER,
                message=f"GraphReadyChunk state {chunk.quality_state.value} blocks trusted extraction.",
                object_type="GraphReadyChunk",
                object_id=object_id,
                route_impact=list(chunk.routes),
            )
        )

    if ChunkRoute.CLAIM_EXTRACTION in chunk.routes and chunk.chunk_type in {
        ChunkType.REFERENCE_ENTRY,
        ChunkType.METADATA,
        ChunkType.ADMINISTRATIVE,
        ChunkType.NOISE,
    }:
        warnings.append(
            _blocker(
                "claim_route_for_non_claim_chunk",
                f"Chunk type {chunk.chunk_type.value} cannot enter claim extraction.",
                "GraphReadyChunk",
                object_id,
                [ChunkRoute.CLAIM_EXTRACTION],
            )
        )

    return warnings


def validate_evidence_path_ref(
    evidence_path: EvidencePathRef,
    *,
    package_paper_id: str,
    chunk_ids: set[str],
    source_element_ids: set[str],
) -> list[QualityWarning]:
    """Return contract violations for one normalized evidence-path reference."""
    warnings: list[QualityWarning] = []
    object_id = evidence_path.evidence_path_id

    if evidence_path.paper_id != package_paper_id:
        warnings.append(
            _blocker(
                "evidence_path_paper_mismatch",
                "EvidencePath paper_id must match package paper_id.",
                "EvidencePathRef",
                object_id,
            )
        )

    if evidence_path.chunk_id not in chunk_ids:
        warnings.append(
            _blocker(
                "evidence_path_missing_chunk",
                "EvidencePath references a missing GraphReadyChunk.",
                "EvidencePathRef",
                object_id,
            )
        )

    missing_elements = [
        element_id
        for element_id in evidence_path.source_element_ids
        if element_id not in source_element_ids
    ]
    if missing_elements:
        warnings.append(
            _blocker(
                "evidence_path_missing_source_element",
                "EvidencePath references missing normalized source elements.",
                "EvidencePathRef",
                object_id,
                evidence={"missing_element_ids": missing_elements[:20]},
            )
        )

    if not evidence_path.source_spans or not any(
        source_span.is_graph_traceable() for source_span in evidence_path.source_spans
    ):
        warnings.append(
            _blocker(
                "evidence_path_missing_traceable_span",
                "EvidencePath must include at least one graph-traceable source span.",
                "EvidencePathRef",
                object_id,
            )
        )

    if evidence_path.quality_state in {GraphReadinessState.REPAIR_REQUIRED, GraphReadinessState.REJECT}:
        warnings.append(
            QualityWarning(
                code="evidence_path_state_blocks_extraction",
                severity=WarningSeverity.REPAIR_REQUIRED
                if evidence_path.quality_state == GraphReadinessState.REPAIR_REQUIRED
                else WarningSeverity.BLOCKER,
                message=f"EvidencePath state {evidence_path.quality_state.value} blocks trusted extraction.",
                object_type="EvidencePathRef",
                object_id=object_id,
                route_impact=[evidence_path.route],
            )
        )

    return warnings


def validate_normalized_package(package: NormalizedPaperPackage) -> ValidationResult:
    """Validate package shape and extraction-gating invariants."""
    warnings: list[QualityWarning] = []

    if package.report.paper_id != package.paper_id:
        warnings.append(
            _blocker(
                "report_paper_mismatch",
                "GraphReadinessReport paper_id must match package paper_id.",
                "NormalizedPaperPackage",
                package.paper_id,
            )
        )

    if package.report.conversion_id != package.conversion_id:
        warnings.append(
            _blocker(
                "report_conversion_mismatch",
                "GraphReadinessReport conversion_id must match package conversion_id.",
                "NormalizedPaperPackage",
                package.paper_id,
            )
        )

    if package.report.document_id != package.document_id:
        warnings.append(
            _blocker(
                "report_document_mismatch",
                "GraphReadinessReport document_id must match package document_id.",
                "NormalizedPaperPackage",
                package.paper_id,
            )
        )

    source_element_ids = _collect_source_element_ids(package)
    chunk_ids = {chunk.chunk_id for chunk in package.chunks}
    evidence_chunk_ids = {path.chunk_id for path in package.evidence_paths}

    for chunk in package.chunks:
        warnings.extend(validate_graph_ready_chunk(chunk))
        if _chunk_allows_extraction(chunk) and chunk.chunk_id not in evidence_chunk_ids:
            warnings.append(
                _blocker(
                    "extraction_chunk_missing_evidence_path",
                    "Extraction-routed GraphReadyChunk must have an EvidencePathRef.",
                    "GraphReadyChunk",
                    chunk.chunk_id,
                    list(chunk.routes),
                )
            )

    for evidence_path in package.evidence_paths:
        warnings.extend(
            validate_evidence_path_ref(
                evidence_path,
                package_paper_id=package.paper_id,
                chunk_ids=chunk_ids,
                source_element_ids=source_element_ids,
            )
        )

    for warning in package.warnings + package.report.blockers:
        warnings.append(warning)

    ok = not any(warning.severity == WarningSeverity.BLOCKER for warning in warnings)
    return ValidationResult(ok=ok, warnings=warnings)


def to_redacted_dict(value: Any) -> Any:
    """Serialize dataclasses/enums while redacting raw text-like fields."""
    if isinstance(value, StrEnum):
        return value.value
    if isinstance(value, Path):
        return str(value)
    if is_dataclass(value) and not isinstance(value, type):
        return to_redacted_dict(asdict(value))
    if isinstance(value, dict):
        redacted: dict[str, Any] = {}
        for key, item in value.items():
            safe_key = str(key)
            if safe_key.lower() in REDACTED_TEXT_KEYS:
                redacted[safe_key] = _hash_redacted_value(item)
            else:
                redacted[safe_key] = to_redacted_dict(item)
        return redacted
    if isinstance(value, list | tuple):
        return [to_redacted_dict(item) for item in value]
    return value


def _hash_redacted_value(value: Any) -> dict[str, str | int]:
    text = str(value)
    return {"sha256": stable_text_hash(text), "length": len(text)}


def _collect_source_element_ids(package: NormalizedPaperPackage) -> set[str]:
    source_element_ids: set[str] = set()
    for collection in (
        package.sections,
        package.paragraphs,
        package.tables,
        package.figures,
        package.equations,
        package.references,
        package.citation_markers,
        package.citations,
    ):
        for element in collection:
            source_element_ids.update(_element_ids(element))
    return source_element_ids


def _element_ids(element: dict[str, Any]) -> set[str]:
    ids: set[str] = set()
    for key, value in element.items():
        if key == "id" or key.endswith("_id"):
            ids.add(str(value))
    return ids


def _chunk_allows_extraction(chunk: GraphReadyChunk) -> bool:
    extraction_routes = {
        ChunkRoute.CLAIM_EXTRACTION,
        ChunkRoute.METHOD_EXTRACTION,
        ChunkRoute.ENTITY_CANDIDATE_EXTRACTION,
        ChunkRoute.RELATION_EXTRACTION,
        ChunkRoute.TABLE_EXTRACTION,
        ChunkRoute.FIGURE_EVIDENCE,
        ChunkRoute.CITATION_GRAPH,
        ChunkRoute.METADATA_GRAPH,
    }
    return bool(set(chunk.routes) & extraction_routes)


def _blocker(
    code: str,
    message: str,
    object_type: str,
    object_id: str,
    route_impact: list[ChunkRoute] | None = None,
    evidence: dict[str, Any] | None = None,
) -> QualityWarning:
    return QualityWarning(
        code=code,
        severity=WarningSeverity.BLOCKER,
        message=message,
        object_type=object_type,
        object_id=object_id,
        route_impact=route_impact or [],
        evidence=evidence or {},
    )


def _repair(
    code: str,
    message: str,
    object_type: str,
    object_id: str,
    route_impact: list[ChunkRoute] | None = None,
    evidence: dict[str, Any] | None = None,
) -> QualityWarning:
    return QualityWarning(
        code=code,
        severity=WarningSeverity.REPAIR_REQUIRED,
        message=message,
        object_type=object_type,
        object_id=object_id,
        route_impact=route_impact or [],
        evidence=evidence or {},
    )


__all__ = [
    "ChunkAnnotation",
    "ChunkRoute",
    "ChunkType",
    "ContentType",
    "CoordinateSpace",
    "EvidencePathRef",
    "ExtractionTrustLevel",
    "GraphReadinessReport",
    "GraphReadinessState",
    "GraphReadyChunk",
    "NormalizedPaperPackage",
    "QualitySignal",
    "QualityWarning",
    "SourceSpan",
    "ValidationResult",
    "WarningSeverity",
    "stable_text_hash",
    "to_redacted_dict",
    "validate_evidence_path_ref",
    "validate_graph_ready_chunk",
    "validate_normalized_package",
]
