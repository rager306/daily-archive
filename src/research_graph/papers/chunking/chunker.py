"""Deterministic structure-aware chunk model for M005/S03.

This module starts the structure-aware chunking path that will replace the
retrieval-only PageIndex/SemanticChunk baseline for import rehearsal. It keeps
all machine-facing outputs redacted: structural spans and identifiers are stored,
but raw paper text, chunk text, embeddings, vectors, and production KG writes are
not emitted.


Formerly: src/arxiv_archive/chunking/chunker.py"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Literal

from research_graph.papers.chunking.figure_units import is_equation_block, is_figure_block
from research_graph.papers.chunking.table_units import is_table_block
from research_graph.repair.chunk_import_contract import (
    EXPECTED_CONTRACT_VERSION,
    EXPECTED_SCHEMA_VERSION,
    RETRIEVAL_ONLY_STATE,
    validate_import_ready_package,
    validation_to_dict,
)

CoordinateSpace = Literal["normalized_markdown"]
GraphReadinessState = Literal["ok_for_graph", "ok_for_retrieval_only", "repair_required", "reject"]
ChunkRoute = Literal[
    "claim_extraction",
    "method_extraction",
    "entity_candidate_extraction",
    "relation_extraction",
    "table_extraction",
    "citation_graph",
    "metadata_graph",
    "retrieval_only",
    "exclude_from_extraction",
]
ChunkType = Literal[
    "claim_candidate",
    "method_candidate",
    "result_candidate",
    "definition_candidate",
    "table_context",
    "table_row_group",
    "figure_caption_context",
    "equation_context",
    "citation_context",
    "reference_entry",
    "metadata",
    "administrative",
    "retrieval_context",
]
AnnotationType = Literal[
    "section_role",
    "route_hint",
    "structural_type",
    "review_blocker",
    "asset_link_hint",
]
ConfidenceClass = Literal["deterministic", "heuristic", "requires_review"]

_HEADING_RE = re.compile(r"^(#{1,6})\s+(.+?)\s*$")
_TABLE_RE = re.compile(r"^\s*\|.*\|\s*$")
_TABLE_SEPARATOR_RE = re.compile(r"^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$")
_FIGURE_RE = re.compile(
    r"^\s*(?:!\[[^\]]*\]\([^)]*\)|(?:fig(?:ure)?\.?\s*\d*[:.]).*)", re.IGNORECASE
)
_EQUATION_RE = re.compile(
    r"^\s*(?:\$\$|\\\[|\\begin\{(?:equation|align|gather|multline)\}|[A-Za-z0-9_{}^\\]+\s*=\s*.+)"
)
_REFERENCE_HEADING_RE = re.compile(r"^(references|bibliography|works cited)$", re.IGNORECASE)


@dataclass(frozen=True)
class StructureAwareMeasurement:
    """One paper's structure-aware package and validation result."""

    paper_id: str
    package: dict[str, Any]
    validation: dict[str, Any]


@dataclass(frozen=True)
class StructureAwareRunResult:
    """Aggregate dry-run result for structure-aware packages."""

    measurements: tuple[StructureAwareMeasurement, ...]
    summary: dict[str, Any]


@dataclass(frozen=True)
class SourceSpan:
    """Absolute span in canonical normalized Markdown."""

    char_start: int
    char_end: int
    coordinate_space: CoordinateSpace = "normalized_markdown"
    page_start: int | None = None
    page_end: int | None = None

    def to_contract(self) -> dict[str, Any]:
        """Serialize a contract source span without raw text."""
        return {
            "coordinate_space": self.coordinate_space,
            "char_start": self.char_start,
            "char_end": self.char_end,
            "page_start": self.page_start,
            "page_end": self.page_end,
        }


@dataclass(frozen=True)
class StructuralElement:
    """One deterministic document element with hierarchy and provenance."""

    element_id: str
    paper_id: str
    element_type: str
    section_path: tuple[str, ...]
    order_index: int
    source_span: SourceSpan
    parent_element_id: str | None = None
    quality_state: GraphReadinessState = "ok_for_retrieval_only"
    warning_codes: tuple[str, ...] = ()

    def to_contract(self) -> dict[str, Any]:
        """Serialize an import-contract element without raw text."""
        return {
            "element_id": self.element_id,
            "paper_id": self.paper_id,
            "element_type": self.element_type,
            "parent_element_id": self.parent_element_id,
            "section_path": list(self.section_path),
            "order_index": self.order_index,
            "source_span": self.source_span.to_contract(),
            "quality_state": self.quality_state,
            "warnings": [
                _warning(code=code, object_id=self.element_id, severity="warn")
                for code in self.warning_codes
            ],
        }


@dataclass(frozen=True)
class RouteEligibility:
    """Allowed and excluded uses for a structure-aware chunk."""

    route: ChunkRoute
    state: GraphReadinessState
    allowed_uses: tuple[str, ...]
    excluded_uses: tuple[str, ...]
    refusal_reasons: tuple[str, ...] = ()


@dataclass(frozen=True)
class StructureAwareChunk:
    """Typed chunk with hierarchy, route eligibility, and source provenance."""

    chunk_id: str
    paper_id: str
    chunk_type: ChunkType
    parent_element_ids: tuple[str, ...]
    section_path: tuple[str, ...]
    order_index: int
    source_span: SourceSpan
    source_artifact: str
    route_eligibility: RouteEligibility
    parent_chunk_id: str | None = None
    evidence_path_id: str | None = None
    warning_codes: tuple[str, ...] = ()

    def to_contract(self) -> dict[str, Any]:
        """Serialize an import-contract chunk without raw text, embeddings, or vectors."""
        return {
            "chunk_id": self.chunk_id,
            "paper_id": self.paper_id,
            "parent_chunk_id": self.parent_chunk_id,
            "parent_element_ids": list(self.parent_element_ids),
            "section_path": list(self.section_path),
            "chunk_type": self.chunk_type,
            "route": self.route_eligibility.route,
            "state": self.route_eligibility.state,
            "allowed_uses": list(self.route_eligibility.allowed_uses),
            "excluded_uses": list(self.route_eligibility.excluded_uses),
            "order_index": self.order_index,
            "source_span": self.source_span.to_contract(),
            "source_artifact": self.source_artifact,
            "evidence_path_id": self.evidence_path_id,
            "quality_warnings": [
                _warning(code=code, object_id=self.chunk_id, severity="warn")
                for code in self.warning_codes
            ],
            "redaction": {
                "raw_text_included": False,
                "chunk_text_included": False,
                "embeddings_included": False,
                "vectors_included": False,
                "secrets_included": False,
            },
        }


@dataclass(frozen=True)
class ChunkAnnotationSidecar:
    """Deterministic annotation sidecar attached to a chunk, never a KG fact."""

    annotation_id: str
    paper_id: str
    chunk_id: str
    method: str
    annotation_type: AnnotationType
    values: dict[str, Any]
    confidence_class: ConfidenceClass
    warning_codes: tuple[str, ...] = ()
    promoted_to_fact: bool = False

    def to_contract(self) -> dict[str, Any]:
        """Serialize a contract annotation without raw text, embeddings, or fact promotion."""
        return {
            "annotation_id": self.annotation_id,
            "paper_id": self.paper_id,
            "chunk_id": self.chunk_id,
            "method": self.method,
            "annotation_type": self.annotation_type,
            "values": self.values,
            "confidence_class": self.confidence_class,
            "promoted_to_fact": False,
            "warnings": [
                _warning(code=code, object_id=self.annotation_id, severity="warn")
                for code in self.warning_codes
            ],
            "redaction": {
                "raw_text_included": False,
                "chunk_text_included": False,
                "embeddings_included": False,
                "vectors_included": False,
                "secrets_included": False,
            },
        }


@dataclass(frozen=True)
class StructureAwarePackage:
    """Redacted package data ready for contract validation."""

    paper_id: str
    title: str | None
    source_artifact: str
    categories: tuple[str, ...] = ()
    elements: tuple[StructuralElement, ...] = field(default_factory=tuple)
    chunks: tuple[StructureAwareChunk, ...] = field(default_factory=tuple)
    annotations: tuple[ChunkAnnotationSidecar, ...] = field(default_factory=tuple)
    run_id: str = "m005-s03-structure-aware"
    created_at: str = field(default_factory=lambda: _now_iso())

    def to_contract(self) -> dict[str, Any]:
        """Serialize the S01 import-ready package shape without raw content."""
        element_records = [element.to_contract() for element in self.elements]
        chunk_records = [chunk.to_contract() for chunk in self.chunks]
        annotation_records = [annotation.to_contract() for annotation in self.annotations]
        diagnostics = _diagnostics_for_package(
            element_records=element_records, chunks=chunk_records, annotations=annotation_records
        )
        return {
            "schema_version": EXPECTED_SCHEMA_VERSION,
            "contract_version": EXPECTED_CONTRACT_VERSION,
            "run_id": self.run_id,
            "created_at": self.created_at,
            "paper_id": self.paper_id,
            "paper": {
                "paper_id": self.paper_id,
                "title": self.title,
                "categories": list(self.categories),
                "source_artifacts": [self.source_artifact],
            },
            "conversion": {
                "conversion_id": f"conversion:{self.paper_id}:structure-aware",
                "converter": "structure_aware_chunking",
                "converter_version": None,
                "source_artifact": self.source_artifact,
                "quality_state": RETRIEVAL_ONLY_STATE,
                "warnings": [],
                "raw_text_included": False,
                "embeddings_included": False,
            },
            "elements": element_records,
            "chunks": chunk_records,
            "annotations": annotation_records,
            "evidence_paths": [],
            "diagnostics": diagnostics,
        }


def parse_markdown_structure(
    markdown: str,
    *,
    paper_id: str,
    title: str | None,
    source_artifact: str,
    categories: tuple[str, ...] = (),
    run_id: str = "m005-s03-structure-aware",
) -> StructureAwarePackage:
    """Parse canonical normalized Markdown into structural elements with absolute spans.

    The parser is intentionally deterministic and conservative. It emits only
    hierarchy, element classes, and source spans; it does not include raw text in
    the returned package.
    """
    root = StructuralElement(
        element_id=f"{paper_id}:document",
        paper_id=paper_id,
        element_type="document",
        section_path=(),
        order_index=0,
        source_span=SourceSpan(char_start=0, char_end=len(markdown)),
    )
    blocks = _markdown_blocks(markdown)
    elements: list[StructuralElement] = [root]
    heading_stack: list[tuple[int, str, str]] = []
    order_index = 1
    for block in blocks:
        heading_match = _HEADING_RE.match(block.text.strip())
        if heading_match:
            level = len(heading_match.group(1))
            heading_text = heading_match.group(2).strip()
            if _looks_administrative(heading_text):
                section_path = tuple(item[2] for item in heading_stack)
                parent_id = heading_stack[-1][1] if heading_stack else root.element_id
                elements.append(
                    StructuralElement(
                        element_id=_element_id(
                            paper_id, order_index, "administrative", heading_text
                        ),
                        paper_id=paper_id,
                        element_type="administrative",
                        parent_element_id=parent_id,
                        section_path=section_path,
                        order_index=order_index,
                        source_span=SourceSpan(char_start=block.start, char_end=block.end),
                    )
                )
                order_index += 1
                continue
            while heading_stack and heading_stack[-1][0] >= level:
                heading_stack.pop()
            section_path = tuple([item[2] for item in heading_stack] + [heading_text])
            parent_id = heading_stack[-1][1] if heading_stack else root.element_id
            element_id = _element_id(paper_id, order_index, "section", heading_text)
            element = StructuralElement(
                element_id=element_id,
                paper_id=paper_id,
                element_type="section",
                parent_element_id=parent_id,
                section_path=section_path,
                order_index=order_index,
                source_span=SourceSpan(char_start=block.start, char_end=block.end),
            )
            elements.append(element)
            heading_stack.append((level, element_id, heading_text))
            order_index += 1
            continue

        section_path = tuple(item[2] for item in heading_stack)
        parent_id = heading_stack[-1][1] if heading_stack else root.element_id
        element_type = _classify_block(block.text, section_path=section_path)
        elements.append(
            StructuralElement(
                element_id=_element_id(
                    paper_id,
                    order_index,
                    element_type,
                    section_path[-1] if section_path else element_type,
                ),
                paper_id=paper_id,
                element_type=element_type,
                parent_element_id=parent_id,
                section_path=section_path,
                order_index=order_index,
                source_span=SourceSpan(char_start=block.start, char_end=block.end),
            )
        )
        order_index += 1
    chunks = tuple(
        _chunk_from_element(element, source_artifact=source_artifact)
        for element in elements
        if element.element_type != "document"
    )
    return StructureAwarePackage(
        paper_id=paper_id,
        title=title,
        source_artifact=source_artifact,
        categories=categories,
        elements=tuple(elements),
        chunks=chunks,
        annotations=tuple(_annotations_for_chunks(chunks)),
        run_id=run_id,
    )


def _annotations_for_chunks(
    chunks: tuple[StructureAwareChunk, ...],
) -> list[ChunkAnnotationSidecar]:
    annotations: list[ChunkAnnotationSidecar] = []
    for chunk in chunks:
        annotations.extend(_annotations_for_chunk(chunk))
    return annotations


def _annotations_for_chunk(chunk: StructureAwareChunk) -> list[ChunkAnnotationSidecar]:
    method = "deterministic_structure_metadata_v1"
    section_role = _section_role(chunk.section_path)
    annotations = [
        ChunkAnnotationSidecar(
            annotation_id=f"{chunk.chunk_id}:annotation:section-role",
            paper_id=chunk.paper_id,
            chunk_id=chunk.chunk_id,
            method=method,
            annotation_type="section_role",
            values={"section_role": section_role, "section_depth": len(chunk.section_path)},
            confidence_class="deterministic",
        ),
        ChunkAnnotationSidecar(
            annotation_id=f"{chunk.chunk_id}:annotation:route-hint",
            paper_id=chunk.paper_id,
            chunk_id=chunk.chunk_id,
            method=method,
            annotation_type="route_hint",
            values={"route": chunk.route_eligibility.route, "state": chunk.route_eligibility.state},
            confidence_class="deterministic",
            warning_codes=chunk.route_eligibility.refusal_reasons,
        ),
        ChunkAnnotationSidecar(
            annotation_id=f"{chunk.chunk_id}:annotation:structural-type",
            paper_id=chunk.paper_id,
            chunk_id=chunk.chunk_id,
            method=method,
            annotation_type="structural_type",
            values={
                "chunk_type": chunk.chunk_type,
                "has_table": chunk.chunk_type in {"table_context", "table_row_group"},
                "has_figure": chunk.chunk_type == "figure_caption_context",
                "has_equation": chunk.chunk_type == "equation_context",
                "has_reference": chunk.chunk_type == "reference_entry",
            },
            confidence_class="deterministic",
        ),
    ]
    if chunk.route_eligibility.refusal_reasons:
        annotations.append(
            ChunkAnnotationSidecar(
                annotation_id=f"{chunk.chunk_id}:annotation:review-blocker",
                paper_id=chunk.paper_id,
                chunk_id=chunk.chunk_id,
                method=method,
                annotation_type="review_blocker",
                values={
                    "reasons": list(chunk.route_eligibility.refusal_reasons),
                    "blocks_trusted_import": True,
                },
                confidence_class="requires_review",
                warning_codes=chunk.route_eligibility.refusal_reasons,
            )
        )
    if chunk.chunk_type in {"table_context", "figure_caption_context"}:
        annotations.append(
            ChunkAnnotationSidecar(
                annotation_id=f"{chunk.chunk_id}:annotation:asset-link-hint",
                paper_id=chunk.paper_id,
                chunk_id=chunk.chunk_id,
                method=method,
                annotation_type="asset_link_hint",
                values={
                    "asset_role": "table" if chunk.chunk_type == "table_context" else "figure",
                    "requires_asset_manifest": True,
                },
                confidence_class="heuristic",
                warning_codes=("asset_manifest_required",),
            )
        )
    return annotations


def _section_role(section_path: tuple[str, ...]) -> str:
    if not section_path:
        return "document"
    label = " ".join(section_path).lower()
    if "abstract" in label:
        return "abstract"
    if "method" in label or "approach" in label:
        return "method"
    if "result" in label or "evaluation" in label:
        return "results"
    if "reference" in label or "bibliography" in label:
        return "references"
    if "introduction" in label:
        return "introduction"
    if "conclusion" in label:
        return "conclusion"
    return "body"


def _chunk_from_element(element: StructuralElement, *, source_artifact: str) -> StructureAwareChunk:
    chunk_type, route, state, refusal_reason = _route_for_element(element)
    return StructureAwareChunk(
        chunk_id=element.element_id.replace(":section:", ":chunk:").replace(
            ":paragraph:", ":chunk:"
        ),
        paper_id=element.paper_id,
        chunk_type=chunk_type,
        parent_element_ids=(element.element_id,),
        section_path=element.section_path,
        order_index=element.order_index,
        source_span=element.source_span,
        source_artifact=source_artifact,
        route_eligibility=RouteEligibility(
            route=route,
            state=state,
            allowed_uses=("routing_diagnostics", "review_only"),
            excluded_uses=("trusted_kg_import", "production_ladybugdb_write"),
            refusal_reasons=(refusal_reason,),
        ),
        warning_codes=(refusal_reason,),
    )


def _route_for_element(
    element: StructuralElement,
) -> tuple[ChunkType, ChunkRoute, GraphReadinessState, str]:
    element_type = element.element_type
    section_label = " ".join(element.section_path).lower()
    if element_type == "reference_entry":
        return (
            "reference_entry",
            "citation_graph",
            "repair_required",
            "citation_route_requires_review",
        )
    if element_type == "table":
        return "table_context", "table_extraction", "repair_required", "table_route_requires_review"
    if element_type == "figure_caption":
        return (
            "figure_caption_context",
            "retrieval_only",
            "repair_required",
            "figure_route_not_import_ready",
        )
    if element_type == "equation":
        return (
            "equation_context",
            "retrieval_only",
            "repair_required",
            "equation_route_not_import_ready",
        )
    if element_type == "administrative":
        return (
            "metadata",
            "metadata_graph",
            "repair_required",
            "administrative_metadata_requires_review",
        )
    if "method" in section_label or "approach" in section_label:
        return (
            "method_candidate",
            "method_extraction",
            "repair_required",
            "method_route_requires_review",
        )
    if any(
        marker in section_label for marker in ("abstract", "result", "conclusion", "introduction")
    ):
        return (
            "claim_candidate",
            "claim_extraction",
            "repair_required",
            "claim_route_requires_review",
        )
    return (
        "retrieval_context",
        "retrieval_only",
        "ok_for_retrieval_only",
        "retrieval_only_not_import_ready",
    )


def empty_structure_aware_package(
    *,
    paper_id: str,
    title: str | None,
    markdown_length: int,
    source_artifact: str,
    categories: tuple[str, ...] = (),
    run_id: str = "m005-s03-structure-aware",
) -> StructureAwarePackage:
    """Create a redacted package skeleton anchored to normalized Markdown length."""
    root_span = SourceSpan(char_start=0, char_end=max(0, markdown_length))
    root = StructuralElement(
        element_id=f"{paper_id}:document",
        paper_id=paper_id,
        element_type="document",
        section_path=(),
        order_index=0,
        source_span=root_span,
        quality_state="ok_for_retrieval_only",
    )
    return StructureAwarePackage(
        paper_id=paper_id,
        title=title,
        source_artifact=source_artifact,
        categories=categories,
        elements=(root,),
        chunks=(),
        run_id=run_id,
    )


def measure_structure_aware_manifest(manifest_path: Path) -> StructureAwareRunResult:
    """Build and validate redacted structure-aware packages for a gold manifest."""
    manifest = json.loads(Path(manifest_path).read_text(encoding="utf-8"))
    run_id = f"m005-s03-structure-aware:{manifest.get('milestone', 'unknown')}"
    measurements: list[StructureAwareMeasurement] = []
    for paper in manifest.get("papers", []):
        if not isinstance(paper, dict):
            continue
        package = build_structure_aware_package_for_paper(paper, run_id=run_id).to_contract()
        validation = validation_to_dict(validate_import_ready_package(package))
        measurements.append(
            StructureAwareMeasurement(
                paper_id=str(package["paper_id"]), package=package, validation=validation
            )
        )
    return StructureAwareRunResult(
        measurements=tuple(measurements), summary=_summary_for_measurements(measurements)
    )


def build_structure_aware_package_for_paper(
    paper: dict[str, Any], *, run_id: str
) -> StructureAwarePackage:
    """Build a structure-aware package for one manifest paper without leaking text."""
    paper_id = str(paper["paper_id"])
    source_path = _select_full_text_path(paper)
    source_artifact = _source_artifact_for_paper(paper, source_path=source_path)
    categories = tuple(
        str(category) for category in paper.get("categories", []) if isinstance(category, str)
    )
    if source_path is None:
        return empty_structure_aware_package(
            paper_id=paper_id,
            title=_string_or_none(paper.get("title")),
            markdown_length=0,
            source_artifact=source_artifact,
            categories=categories,
            run_id=run_id,
        )
    markdown = source_path.read_text(encoding="utf-8")
    return parse_markdown_structure(
        markdown,
        paper_id=paper_id,
        title=_string_or_none(paper.get("title")),
        source_artifact=source_artifact,
        categories=categories,
        run_id=run_id,
    )


def write_structure_aware_run(result: StructureAwareRunResult, output_dir: Path) -> None:
    """Write redacted structure-aware summary and package diagnostics."""
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "structure-aware-summary.json").write_text(
        json.dumps(result.summary, indent=2, sort_keys=True),
        encoding="utf-8",
    )
    (output_dir / "structure-aware-package-diagnostics.jsonl").write_text(
        "".join(
            json.dumps(_measurement_to_record(measurement), sort_keys=True) + "\n"
            for measurement in result.measurements
        ),
        encoding="utf-8",
    )


def _measurement_to_record(measurement: StructureAwareMeasurement) -> dict[str, Any]:
    package = measurement.package
    diagnostics = package["diagnostics"]
    return {
        "schema_version": "m005-structure-aware-package-diagnostic.v1",
        "paper_id": measurement.paper_id,
        "valid_package": measurement.validation["valid_package"],
        "import_ready": measurement.validation["import_ready"],
        "import_eligible_chunk_count": measurement.validation["import_eligible_chunk_count"],
        "refused_chunk_count": measurement.validation["refused_chunk_count"],
        "element_count": len(package["elements"]),
        "chunk_count": len(package["chunks"]),
        "counts_by_state": diagnostics["counts_by_state"],
        "counts_by_route": diagnostics["counts_by_route"],
        "counts_by_chunk_type": diagnostics["counts_by_chunk_type"],
        "refusal_counts": diagnostics["refusal_counts"],
        "annotation_count": len(package["annotations"]),
        "annotation_counts_by_type": diagnostics["annotation_counts_by_type"],
        "annotation_counts_by_confidence": diagnostics["annotation_counts_by_confidence"],
        "annotation_warning_counts": diagnostics["annotation_warning_counts"],
        "chunk_diagnostics": _redacted_chunk_diagnostics(package),
        "source_span_coverage": diagnostics["source_span_coverage"],
        "parent_reference_resolution_rate": diagnostics["parent_reference_resolution_rate"],
        "raw_text_included": False,
        "embeddings_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


def _redacted_chunk_diagnostics(package: dict[str, Any]) -> list[dict[str, Any]]:
    """Return chunk-level machine evidence without raw text or embeddings."""
    records: list[dict[str, Any]] = []
    for chunk in package.get("chunks", []):
        if not isinstance(chunk, dict):
            continue
        records.append(
            {
                "chunk_id": chunk.get("chunk_id"),
                "chunk_type": chunk.get("chunk_type"),
                "route": chunk.get("route"),
                "state": chunk.get("state"),
                "source_span": chunk.get("source_span"),
                "parent_element_ids": list(chunk.get("parent_element_ids", [])),
                "section_path": list(chunk.get("section_path", [])),
                "refusal_reasons": [
                    str(warning.get("code"))
                    for warning in chunk.get("quality_warnings", [])
                    if isinstance(warning, dict)
                ],
            }
        )
    return records


def _summary_for_measurements(measurements: list[StructureAwareMeasurement]) -> dict[str, Any]:
    counts_by_state: dict[str, int] = {}
    counts_by_route: dict[str, int] = {}
    counts_by_chunk_type: dict[str, int] = {}
    refusal_counts: dict[str, int] = {}
    annotation_counts_by_type: dict[str, int] = {}
    annotation_counts_by_confidence: dict[str, int] = {}
    annotation_warning_counts: dict[str, int] = {}
    for measurement in measurements:
        diagnostics = measurement.package["diagnostics"]
        _merge_counts(counts_by_state, diagnostics.get("counts_by_state", {}))
        _merge_counts(counts_by_route, diagnostics.get("counts_by_route", {}))
        _merge_counts(counts_by_chunk_type, diagnostics.get("counts_by_chunk_type", {}))
        _merge_counts(refusal_counts, diagnostics.get("refusal_counts", {}))
        _merge_counts(annotation_counts_by_type, diagnostics.get("annotation_counts_by_type", {}))
        _merge_counts(
            annotation_counts_by_confidence, diagnostics.get("annotation_counts_by_confidence", {})
        )
        _merge_counts(annotation_warning_counts, diagnostics.get("annotation_warning_counts", {}))
    return {
        "schema_version": "m005-structure-aware-run.v1",
        "paper_count": len(measurements),
        "valid_package_count": sum(
            1 for measurement in measurements if measurement.validation["valid_package"]
        ),
        "import_ready_count": sum(
            1 for measurement in measurements if measurement.validation["import_ready"]
        ),
        "import_eligible_chunk_count": sum(
            int(measurement.validation["import_eligible_chunk_count"])
            for measurement in measurements
        ),
        "refused_chunk_count": sum(
            int(measurement.validation["refused_chunk_count"]) for measurement in measurements
        ),
        "element_count": sum(len(measurement.package["elements"]) for measurement in measurements),
        "chunk_count": sum(len(measurement.package["chunks"]) for measurement in measurements),
        "counts_by_state": dict(sorted(counts_by_state.items())),
        "counts_by_route": dict(sorted(counts_by_route.items())),
        "counts_by_chunk_type": dict(sorted(counts_by_chunk_type.items())),
        "refusal_counts": dict(sorted(refusal_counts.items())),
        "annotation_count": sum(
            len(measurement.package["annotations"]) for measurement in measurements
        ),
        "annotation_counts_by_type": dict(sorted(annotation_counts_by_type.items())),
        "annotation_counts_by_confidence": dict(sorted(annotation_counts_by_confidence.items())),
        "annotation_warning_counts": dict(sorted(annotation_warning_counts.items())),
        "raw_text_included": False,
        "embeddings_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
        "claims": [
            "structure_aware_dry_run_only",
            "current_structure_aware_chunks_are_not_claimed_import_ready",
            "production_kg_writes_remain_blocked",
        ],
    }


def _select_full_text_path(paper: dict[str, Any]) -> Path | None:
    for raw_path in paper.get("required_paths", []):
        path = Path(str(raw_path))
        if path.name == "full_text.md" and path.exists():
            return path
        full_text_path = path / "full_text.md"
        if path.is_dir() and full_text_path.exists():
            return full_text_path
    fallback = Path("/root/.research/papers") / str(paper["paper_id"]) / "full_text.md"
    return fallback if fallback.exists() else None


def _source_artifact_for_paper(paper: dict[str, Any], *, source_path: Path | None) -> str:
    if source_path is not None:
        return str(source_path)
    artifacts = paper.get("source_artifacts")
    if isinstance(artifacts, list) and artifacts:
        return str(artifacts[0])
    return f"normalized_markdown:{paper['paper_id']}"


def _merge_counts(target: dict[str, int], source: Any) -> None:
    if not isinstance(source, dict):
        return
    for key, value in source.items():
        target[str(key)] = target.get(str(key), 0) + int(value)


def _string_or_none(value: Any) -> str | None:
    return str(value) if value is not None else None


def main(argv: list[str] | None = None) -> int:
    """CLI entry point for redacted structure-aware dry runs."""
    parser = argparse.ArgumentParser(
        description="Validate structure-aware chunks against the M005 import-ready contract."
    )
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args(argv)
    result = measure_structure_aware_manifest(args.manifest)
    write_structure_aware_run(result, args.output_dir)
    sys.stdout.write(json.dumps(result.summary, indent=2, sort_keys=True))
    sys.stdout.write("\n")
    return 0


@dataclass(frozen=True)
class _MarkdownBlock:
    text: str
    start: int
    end: int


def _markdown_blocks(markdown: str) -> list[_MarkdownBlock]:
    blocks: list[_MarkdownBlock] = []
    block_start: int | None = None
    block_lines: list[str] = []
    position = 0
    for line in markdown.splitlines(keepends=True):
        line_start = position
        line_end = position + len(line)
        position = line_end
        if line.strip():
            if block_start is None:
                block_start = line_start
            block_lines.append(line)
            continue
        if block_start is not None:
            blocks.append(
                _MarkdownBlock(text="".join(block_lines), start=block_start, end=line_start)
            )
            block_start = None
            block_lines = []
    if block_start is not None:
        blocks.append(
            _MarkdownBlock(text="".join(block_lines), start=block_start, end=len(markdown))
        )
    return blocks


def _classify_block(text: str, *, section_path: tuple[str, ...]) -> str:
    stripped = text.strip()
    section_name = section_path[-1] if section_path else ""
    if _REFERENCE_HEADING_RE.match(section_name) or re.match(r"^\s*\[?\d+\]?\s*[.)]?\s+", stripped):
        return "reference_entry"
    if is_table_block(stripped):
        return "table"
    if is_figure_block(stripped):
        return "figure_caption"
    if is_equation_block(stripped):
        return "equation"
    if _looks_administrative(stripped):
        return "administrative"
    return "paragraph"


def _looks_administrative(text: str) -> bool:
    lowered = text.lower()
    administrative_markers = (
        "orcid",
        "correspondence:",
        "submission history",
        "access paper",
        "bookmark",
        "bibtex",
        "computer science >",
    )
    return any(marker in lowered for marker in administrative_markers)


def _element_id(paper_id: str, order_index: int, element_type: str, label: str) -> str:
    return f"{paper_id}:{order_index:04d}:{_slug(element_type)}:{_slug(label)}"


def _slug(value: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")
    return slug or "item"


def _diagnostics_for_package(
    *,
    element_records: list[dict[str, Any]],
    chunks: list[dict[str, Any]],
    annotations: list[dict[str, Any]],
) -> dict[str, Any]:
    element_ids = {
        str(element.get("element_id"))
        for element in element_records
        if element.get("element_id") is not None
    }
    source_span_count = sum(1 for chunk in chunks if _valid_source_span(chunk.get("source_span")))
    parent_resolved_count = sum(
        1
        for chunk in chunks
        if chunk.get("parent_element_ids")
        and all(str(parent_id) in element_ids for parent_id in chunk.get("parent_element_ids", []))
    )
    import_eligible_count = sum(
        1
        for chunk in chunks
        if chunk["state"] == "ok_for_graph" and "trusted_kg_import" in chunk["allowed_uses"]
    )
    refused_count = len(chunks) - import_eligible_count
    refusal_counts: dict[str, int] = {}
    for chunk in chunks:
        for warning in chunk.get("quality_warnings", []):
            reason = str(warning.get("code", "unspecified_refusal"))
            refusal_counts[reason] = refusal_counts.get(reason, 0) + 1
    annotation_warning_counts: dict[str, int] = {}
    for annotation in annotations:
        for warning in annotation.get("warnings", []):
            reason = str(warning.get("code", "unspecified_annotation_warning"))
            annotation_warning_counts[reason] = annotation_warning_counts.get(reason, 0) + 1
    return {
        "package_state": RETRIEVAL_ONLY_STATE,
        "valid_package": True,
        "import_eligible_chunk_count": import_eligible_count,
        "refused_chunk_count": refused_count,
        "counts_by_state": _counts(chunk["state"] for chunk in chunks),
        "counts_by_route": _counts(chunk["route"] for chunk in chunks),
        "counts_by_chunk_type": _counts(chunk["chunk_type"] for chunk in chunks),
        "refusal_counts": dict(sorted(refusal_counts.items())),
        "annotation_counts_by_type": _counts(
            annotation["annotation_type"] for annotation in annotations
        ),
        "annotation_counts_by_confidence": _counts(
            annotation["confidence_class"] for annotation in annotations
        ),
        "annotation_warning_counts": dict(sorted(annotation_warning_counts.items())),
        "source_span_coverage": source_span_count / len(chunks) if chunks else 0.0,
        "parent_reference_resolution_rate": parent_resolved_count / len(chunks) if chunks else 0.0,
        "evidence_path_resolution_rate": 0.0,
        "raw_text_included": False,
        "embeddings_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


def _valid_source_span(value: Any) -> bool:
    return (
        isinstance(value, dict)
        and value.get("coordinate_space") == "normalized_markdown"
        and isinstance(value.get("char_start"), int)
        and isinstance(value.get("char_end"), int)
        and value["char_start"] <= value["char_end"]
    )


def _warning(*, code: str, object_id: str, severity: str) -> dict[str, Any]:
    return {
        "code": code,
        "severity": severity,
        "message": code.replace("_", " "),
        "object_id": object_id,
        "route": None,
        "blocks_import": severity in {"error", "blocker"},
    }


def _counts(values: Any) -> dict[str, int]:
    counts: dict[str, int] = {}
    for value in values:
        counts[str(value)] = counts.get(str(value), 0) + 1
    return dict(sorted(counts.items()))


def _now_iso() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat()


if __name__ == "__main__":
    raise SystemExit(main())
