"""Deterministic structure-aware chunk model for M005/S03.

This module starts the structure-aware chunking path that will replace the
retrieval-only PageIndex/SemanticChunk baseline for import rehearsal. The first
slice task defines stable data shapes and a redacted package builder only; later
S03 tasks fill in Markdown parsing, route assignment, and gold-corpus runs.
"""

from __future__ import annotations

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


def _diagnostics_for_chunks(chunks: list[dict[str, Any]]) -> dict[str, Any]:
    import_eligible_count = sum(1 for chunk in chunks if chunk["state"] == "ok_for_graph" and "trusted_kg_import" in chunk["allowed_uses"])
    refused_count = len(chunks) - import_eligible_count
    return {
        "package_state": RETRIEVAL_ONLY_STATE,
        "valid_package": True,
        "import_eligible_chunk_count": import_eligible_count,
        "refused_chunk_count": refused_count,
        "counts_by_state": _counts(chunk["state"] for chunk in chunks),
        "counts_by_route": _counts(chunk["route"] for chunk in chunks),
        "counts_by_chunk_type": _counts(chunk["chunk_type"] for chunk in chunks),
        "refusal_counts": {},
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
