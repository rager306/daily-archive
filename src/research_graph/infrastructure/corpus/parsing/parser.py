"""Deterministic parser for full-text ingestion results.

The parser boundary converts source text into typed, hierarchy-aware article
elements. It deliberately performs no I/O, network access, model calls, or
storage writes; ingestion remains responsible for obtaining bytes/text.


Formerly: src/arxiv_archive/parsing/parser.py"""

from __future__ import annotations

import re
from dataclasses import replace
from typing import NamedTuple

from research_graph.infrastructure.corpus.ingestion import FullTextIngestionResult
from research_graph.infrastructure.corpus.parsing.normalization import (
    slugify,
    strip_yaml_frontmatter,
)
from research_graph.infrastructure.corpus.parsing.structure import (
    ParsedArticle,
    ParsedArticleElement,
)

PARSER_VERSION = "markdown_headings_v1"
_HEADING_RE = re.compile(r"^(#{1,6})\s+(.+?)\s*$")


class _HeadingSection(NamedTuple):
    level: int
    title: str
    text: str


def parse_article(ingestion: FullTextIngestionResult) -> ParsedArticle:
    """Parse an ingestion result into deterministic, typed article elements."""
    sections = _parse_heading_sections(ingestion.text)
    if not sections:
        return _fallback_article(ingestion)

    elements: list[ParsedArticleElement] = []
    stack: list[ParsedArticleElement] = []
    used_ids: set[str] = set()

    for order, section in enumerate(sections):
        parent = _nearest_parent(stack, section.level)
        element_id = _stable_element_id(ingestion.paper_id, section.title, used_ids, root=order == 0)
        path = [element_id] if parent is None else [*parent.path, element_id]
        element = ParsedArticleElement(
            id=element_id,
            paper_id=ingestion.paper_id,
            kind="section",
            title=section.title,
            level=section.level,
            order=order,
            text=section.text,
            source_path=ingestion.source_path,
            path=path,
            parent_id=parent.id if parent is not None else None,
            provenance={
                "paper_id": ingestion.paper_id,
                "source_path": str(ingestion.source_path),
                "heading_level": str(section.level),
                "parser": PARSER_VERSION,
                "parse_fallback": "false",
            },
        )
        elements.append(element)
        stack = [candidate for candidate in stack if candidate.level < element.level]
        stack.append(element)

    provenance = dict(ingestion.provenance)
    provenance["parser"] = PARSER_VERSION
    provenance["parse_fallback"] = "false"
    provenance["section_count"] = str(len(elements))
    return ParsedArticle(
        paper_id=ingestion.paper_id,
        source_path=ingestion.source_path,
        elements=elements,
        validation_warnings=[],
        provenance=provenance,
    )


def _parse_heading_sections(text: str) -> list[_HeadingSection]:
    body = strip_yaml_frontmatter(text)
    lines = body.splitlines()
    heading_positions: list[tuple[int, int, str]] = []
    for index, line in enumerate(lines):
        match = _HEADING_RE.match(line)
        if match:
            heading_positions.append((index, len(match.group(1)), match.group(2).strip()))

    sections: list[_HeadingSection] = []
    for position, (line_index, level, title) in enumerate(heading_positions):
        next_line_index = (
            heading_positions[position + 1][0]
            if position + 1 < len(heading_positions)
            else len(lines)
        )
        section_text = "\n".join(lines[line_index + 1 : next_line_index]).strip()
        sections.append(_HeadingSection(level=level, title=title, text=section_text))
    return sections


def _nearest_parent(stack: list[ParsedArticleElement], level: int) -> ParsedArticleElement | None:
    candidates = [element for element in stack if element.level < level]
    return candidates[-1] if candidates else None


def _stable_element_id(paper_id: str, title: str, used_ids: set[str], *, root: bool) -> str:
    if root:
        element_id = f"{paper_id}:root"
    else:
        element_id = f"{paper_id}:{slugify(title)}"
    if element_id not in used_ids:
        used_ids.add(element_id)
        return element_id

    suffix = 2
    while f"{element_id}-{suffix}" in used_ids:
        suffix += 1
    deduped = f"{element_id}-{suffix}"
    used_ids.add(deduped)
    return deduped


def _fallback_article(ingestion: FullTextIngestionResult) -> ParsedArticle:
    root_id = f"{ingestion.paper_id}:root"
    fallback_id = f"{ingestion.paper_id}:full-text"
    shared_provenance = {
        "paper_id": ingestion.paper_id,
        "source_path": str(ingestion.source_path),
        "parser": PARSER_VERSION,
        "fallback_reason": "no_headings",
        "parse_fallback": "true",
    }
    root = ParsedArticleElement(
        id=root_id,
        paper_id=ingestion.paper_id,
        kind="section",
        title="Document",
        level=1,
        order=0,
        text="",
        source_path=ingestion.source_path,
        path=[root_id],
        parent_id=None,
        provenance=dict(shared_provenance),
    )
    fallback = ParsedArticleElement(
        id=fallback_id,
        paper_id=ingestion.paper_id,
        kind="section",
        title="Full Text",
        level=2,
        order=1,
        text=ingestion.text,
        source_path=ingestion.source_path,
        path=[root_id, fallback_id],
        parent_id=root_id,
        provenance=dict(shared_provenance),
    )
    provenance = dict(ingestion.provenance)
    provenance["parser"] = PARSER_VERSION
    provenance["fallback_reason"] = "no_headings"
    provenance["parse_fallback"] = "true"
    provenance["section_count"] = "2"
    return ParsedArticle(
        paper_id=ingestion.paper_id,
        source_path=ingestion.source_path,
        elements=[root, fallback],
        validation_warnings=["no markdown headings found; created fallback full-text section"],
        provenance=provenance,
    )


def with_parse_warning(article: ParsedArticle, warning: str) -> ParsedArticle:
    """Return a parsed article with an additional immutable warning."""
    return replace(article, validation_warnings=[*article.validation_warnings, warning])


__all__ = [
    "PARSER_VERSION",
    "ParsedArticle",
    "ParsedArticleElement",
    "parse_article",
    "with_parse_warning",
]
