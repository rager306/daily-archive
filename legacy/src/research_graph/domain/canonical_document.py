"""CanonicalDocument IR (M275 evidence foundation).

Domain-pure intermediate representation between immutable parser artifacts
(PDF / TEI / ODL JSON) and projections (markdown body, graph candidates).

Never authorizes import or graph writes.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal

SCHEMA_VERSION = "canonical-document.v1"

BlockKind = Literal[
    "section",
    "paragraph",
    "heading",
    "table",
    "figure",
    "equation",
    "caption",
    "list_item",
    "reference",
    "other",
]


@dataclass(frozen=True, slots=True)
class SourceSpanRef:
    """Grounding pointer into a parser artifact (no source text payload)."""

    artifact_role: str  # pdf | tei | odl_layout | markdown
    artifact_hash: str | None = None
    page: int | None = None
    bbox: tuple[float, float, float, float] | None = None
    element_id: str | None = None
    char_start: int | None = None
    char_end: int | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "artifact_role": self.artifact_role,
            "artifact_hash": self.artifact_hash,
            "page": self.page,
            "bbox": list(self.bbox) if self.bbox is not None else None,
            "element_id": self.element_id,
            "char_start": self.char_start,
            "char_end": self.char_end,
        }


@dataclass(frozen=True, slots=True)
class CanonicalBlock:
    block_id: str
    kind: BlockKind
    text: str
    level: int = 0
    spans: tuple[SourceSpanRef, ...] = ()
    meta: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "block_id": self.block_id,
            "kind": self.kind,
            "text": self.text,
            "level": self.level,
            "spans": [s.to_dict() for s in self.spans],
            "meta": dict(self.meta),
        }


@dataclass(frozen=True, slots=True)
class CanonicalSection:
    section_id: str
    title: str
    level: int
    blocks: tuple[CanonicalBlock, ...] = ()
    children: tuple[CanonicalSection, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "section_id": self.section_id,
            "title": self.title,
            "level": self.level,
            "blocks": [b.to_dict() for b in self.blocks],
            "children": [c.to_dict() for c in self.children],
        }


@dataclass(frozen=True, slots=True)
class CanonicalDocument:
    """Document IR; markdown is not required to be the source of truth."""

    schema_version: str
    paper_id: str
    title: str | None
    sections: tuple[CanonicalSection, ...]
    blocks: tuple[CanonicalBlock, ...]  # flat index for search
    parser_runs: tuple[dict[str, Any], ...]
    source_hashes: dict[str, str]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("CanonicalDocument cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "paper_id": self.paper_id,
            "title": self.title,
            "sections": [s.to_dict() for s in self.sections],
            "blocks": [b.to_dict() for b in self.blocks],
            "parser_runs": list(self.parser_runs),
            "source_hashes": dict(self.source_hashes),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "CanonicalDocument.v1 IR. Projections (markdown) are derived views. "
                "Never import authority."
            ),
        }


__all__ = [
    "SCHEMA_VERSION",
    "BlockKind",
    "SourceSpanRef",
    "CanonicalBlock",
    "CanonicalSection",
    "CanonicalDocument",
]
