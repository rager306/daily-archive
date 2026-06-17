"""Structured parser output contracts for article text."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path


@dataclass(frozen=True)
class ParsedArticleElement:
    """One deterministic article element emitted by the parser boundary."""

    id: str
    paper_id: str
    kind: str
    title: str
    level: int
    order: int
    text: str
    source_path: Path
    path: list[str]
    parent_id: str | None
    provenance: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class ParsedArticle:
    """Parser output before PageIndex construction."""

    paper_id: str
    source_path: Path
    elements: list[ParsedArticleElement]
    validation_warnings: list[str]
    provenance: dict[str, str]

    @property
    def root(self) -> ParsedArticleElement:
        """Return the first parsed element, which is the PageIndex root candidate."""
        return self.elements[0]


__all__ = ["ParsedArticle", "ParsedArticleElement"]
