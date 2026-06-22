"""Domain typed models: SemanticChunk + EvidencePath (D086, canonical home).

Pure data contracts (frozen dataclasses) for the knowledge pipeline. The
build/validate logic that produces these lives in
:mod:`research_graph.papers.semantic_chunks` (infrastructure); only the model
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
    """Trace from paper to PageIndexNode to SemanticChunk."""

    paper_id: str
    page_index_node_id: str
    semantic_chunk_id: str
    node_path: list[str]
    validation_warnings: list[str] = field(default_factory=list)
    provenance: dict[str, str] = field(default_factory=dict)


__all__ = ["EvidencePath", "SemanticChunk"]
