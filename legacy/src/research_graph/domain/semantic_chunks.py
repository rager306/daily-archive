"""Domain typed models: SemanticChunk + EvidencePath (D086, canonical home).

Pure data contracts (frozen dataclasses) for the knowledge pipeline. The
build/validate logic that produces these lives in
:mod:`research_graph.infrastructure.papers.semantic_chunks` (infrastructure); only the model
types belong here so the domain Core and the application Ports can reference
them without importing infrastructure.

Canonical home moved from ``papers.semantic_chunks`` per D086 (schema evolution,
not duplication — §6.3 #6); the old module re-exports these for back-compat.
"""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True)
class SemanticChunk:
    """A deterministic text chunk attached to one PageIndexNode."""

    id: str
    paper_id: str
    page_index_node_id: str
    page_index_path: list[str]
    order: int
    text: str
    char_start: int
    char_end: int
    chunking_strategy: str
    validation_warnings: list[str] = field(default_factory=list)
    provenance: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class EvidencePath:
    """Trace from paper to PageIndexNode to SemanticChunk.

    M276: optional grounding fields (page/bbox/artifact_hash/element_id/char
    range) for evidence-trace. Defaults preserve all existing constructors.
    Never implies import eligibility.
    """

    paper_id: str
    page_index_node_id: str
    semantic_chunk_id: str
    node_path: list[str]
    validation_warnings: list[str] = field(default_factory=list)
    provenance: dict[str, str] = field(default_factory=dict)
    # Optional grounding (E1.5) — additive; Ladybug schema may ignore until E4.
    artifact_hash: str | None = None
    page: int | None = None
    bbox: tuple[float, float, float, float] | None = None
    element_id: str | None = None
    char_start: int | None = None
    char_end: int | None = None

    def grounding_dict(self) -> dict[str, object]:
        """Span-like mapping for resolvability checks (import never authorized)."""
        return {
            "artifact_hash": self.artifact_hash,
            "page": self.page,
            "bbox": list(self.bbox) if self.bbox is not None else None,
            "element_id": self.element_id,
            "char_start": self.char_start,
            "char_end": self.char_end,
        }


__all__ = ["EvidencePath", "SemanticChunk"]
