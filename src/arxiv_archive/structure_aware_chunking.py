"""Deterministic structure-aware chunk model for M005/S03.

This module starts the structure-aware chunking path that will replace the
retrieval-only PageIndex/SemanticChunk baseline for import rehearsal. It keeps
all machine-facing outputs redacted: structural spans and identifiers are stored,
but raw paper text, chunk text, embeddings, vectors, and production KG writes are
not emitted.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from datetime import UTC, datetime
from typing import Any, Literal

from arxiv_archive.chunk_import_contract import (
    EXPECTED_CONTRACT_VERSION,
    EXPECTED_SCHEMA_VERSION,
    RETRIEVAL_ONLY_STATE,
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

_HEADING_RE = re.compile(r"^(#{1,6})\s+(.+?)\s*$")
_TABLE_RE = re.compile(r"^\s*\|.*\|\s*$")
_TABLE_SEPARATOR_RE = re.compile(r"^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$")
_FIGURE_RE = re.compile(r"^\s*(?:!\[[^\]]*\]\([^)]*\)|(?:fig(?:ure)?\.?\s*\d*[:.]).*)", re.IGNORECASE)
_EQUATION_RE = re.compile(r"^\s*(?:\$\$|\\\[|\\begin\{(?:equation|align|gather|multline)\}|[A-Za-z0-9_{}^\\]+\s*=\s*.+)")
_REFERENCE_HEADING_RE = re.compile(r"^(references|bibliography|works cited)$", re.IGNORECASE)


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
            "warnings": [_warning(code=code, object_id=self.element_id, severity="warn") for code in self.warning_codes],
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
            "quality_warnings": [_warning(code=code, object_id=self.chunk_id, severity="warn") for code in self.warning_codes],
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
    elements: tuple[StructuralElement, ...] = field(default_factory=tuple)
    chunks: tuple[StructureAwareChunk, ...] = field(default_factory=tuple)
    run_id: str = "m005-s03-structure-aware"
    created_at: str = field(default_factory=lambda: _now_iso())

    def to_contract(self) -> dict[str, Any]:
        """Serialize the S01 import-ready package shape without raw content."""
        element_records = [element.to_contract() for element in self.elements]
        chunk_records = [chunk.to_contract() for chunk in self.chunks]
        diagnostics = _diagnostics_for_chunks(chunk_records)
        return {
            "schema_version": EXPECTED_SCHEMA_VERSION,
            "contract_version": EXPECTED_CONTRACT_VERSION,
            "run_id": self.run_id,
            "created_at": self.created_at,
            "paper_id": self.paper_id,
            "paper": {
                "paper_id": self.paper_id,
                "title": self.title,
                "categories": [],
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
            "annotations": [],
            "evidence_paths": [],
            "diagnostics": diagnostics,
        }


def parse_markdown_structure(
    markdown: str,
    *,
    paper_id: str,
    title: str | None,
    source_artifact: str,
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
                        element_id=_element_id(paper_id, order_index, "administrative", heading_text),
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
                element_id=_element_id(paper_id, order_index, element_type, section_path[-1] if section_path else element_type),
                paper_id=paper_id,
                element_type=element_type,
                parent_element_id=parent_id,
                section_path=section_path,
                order_index=order_index,
                source_span=SourceSpan(char_start=block.start, char_end=block.end),
            )
        )
        order_index += 1
    return StructureAwarePackage(
        paper_id=paper_id,
        title=title,
        source_artifact=source_artifact,
        elements=tuple(elements),
        chunks=tuple(_chunk_from_element(element, source_artifact=source_artifact) for element in elements if element.element_type != "document"),
        run_id=run_id,
    )


def _chunk_from_element(element: StructuralElement, *, source_artifact: str) -> StructureAwareChunk:
    chunk_type, route, state, refusal_reason = _route_for_element(element)
    return StructureAwareChunk(
        chunk_id=element.element_id.replace(":section:", ":chunk:").replace(":paragraph:", ":chunk:"),
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


def _route_for_element(element: StructuralElement) -> tuple[ChunkType, ChunkRoute, GraphReadinessState, str]:
    element_type = element.element_type
    section_label = " ".join(element.section_path).lower()
    if element_type == "reference_entry":
        return "reference_entry", "citation_graph", "repair_required", "citation_route_requires_review"
    if element_type == "table":
        return "table_context", "table_extraction", "repair_required", "table_route_requires_review"
    if element_type == "figure_caption":
        return "figure_caption_context", "retrieval_only", "repair_required", "figure_route_not_import_ready"
    if element_type == "equation":
        return "equation_context", "retrieval_only", "repair_required", "equation_route_not_import_ready"
    if element_type == "administrative":
        return "metadata", "metadata_graph", "repair_required", "administrative_metadata_requires_review"
    if "method" in section_label or "approach" in section_label:
        return "method_candidate", "method_extraction", "repair_required", "method_route_requires_review"
    if any(marker in section_label for marker in ("abstract", "result", "conclusion", "introduction")):
        return "claim_candidate", "claim_extraction", "repair_required", "claim_route_requires_review"
    return "retrieval_context", "retrieval_only", "ok_for_retrieval_only", "retrieval_only_not_import_ready"


def empty_structure_aware_package(
    *,
    paper_id: str,
    title: str | None,
    markdown_length: int,
    source_artifact: str,
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
        elements=(root,),
        chunks=(),
        run_id=run_id,
    )


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
            blocks.append(_MarkdownBlock(text="".join(block_lines), start=block_start, end=line_start))
            block_start = None
            block_lines = []
    if block_start is not None:
        blocks.append(_MarkdownBlock(text="".join(block_lines), start=block_start, end=len(markdown)))
    return blocks


def _classify_block(text: str, *, section_path: tuple[str, ...]) -> str:
    stripped = text.strip()
    section_name = section_path[-1] if section_path else ""
    if _REFERENCE_HEADING_RE.match(section_name) or re.match(r"^\s*\[?\d+\]?\s*[.)]?\s+", stripped):
        return "reference_entry"
    if any(_TABLE_RE.match(line) for line in stripped.splitlines()) or any(_TABLE_SEPARATOR_RE.match(line) for line in stripped.splitlines()):
        return "table"
    if _FIGURE_RE.match(stripped):
        return "figure_caption"
    if _EQUATION_RE.match(stripped):
        return "equation"
    if _looks_administrative(stripped):
        return "administrative"
    return "paragraph"


def _looks_administrative(text: str) -> bool:
    lowered = text.lower()
    administrative_markers = ("orcid", "correspondence:", "submission history", "access paper", "bookmark", "bibtex", "computer science >")
    return any(marker in lowered for marker in administrative_markers)


def _element_id(paper_id: str, order_index: int, element_type: str, label: str) -> str:
    return f"{paper_id}:{order_index:04d}:{_slug(element_type)}:{_slug(label)}"


def _slug(value: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")
    return slug or "item"


def _diagnostics_for_chunks(chunks: list[dict[str, Any]]) -> dict[str, Any]:
    import_eligible_count = sum(1 for chunk in chunks if chunk["state"] == "ok_for_graph" and "trusted_kg_import" in chunk["allowed_uses"])
    refused_count = len(chunks) - import_eligible_count
    refusal_counts: dict[str, int] = {}
    for chunk in chunks:
        for warning in chunk.get("quality_warnings", []):
            reason = str(warning.get("code", "unspecified_refusal"))
            refusal_counts[reason] = refusal_counts.get(reason, 0) + 1
    return {
        "package_state": RETRIEVAL_ONLY_STATE,
        "valid_package": True,
        "import_eligible_chunk_count": import_eligible_count,
        "refused_chunk_count": refused_count,
        "counts_by_state": _counts(chunk["state"] for chunk in chunks),
        "counts_by_route": _counts(chunk["route"] for chunk in chunks),
        "counts_by_chunk_type": _counts(chunk["chunk_type"] for chunk in chunks),
        "refusal_counts": dict(sorted(refusal_counts.items())),
        "source_span_coverage": 1.0 if chunks else 0.0,
        "parent_reference_resolution_rate": 1.0 if chunks else 0.0,
        "evidence_path_resolution_rate": 0.0,
        "raw_text_included": False,
        "embeddings_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


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
