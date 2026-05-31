"""Regression coverage for parser and PageIndex boundary contracts.

These tests exercise generated markdown structures plus a real checked-in article
fixture so hierarchy, path coverage, node summaries, and fallback decisions are
visible when later refactors drift.
"""

from __future__ import annotations

import json
from pathlib import Path

from hypothesis import given, settings
from hypothesis import strategies as st

from arxiv_archive.full_text import (
    FullTextIngestionResult,
    FullTextQualityReport,
    FullTextSource,
    ingest_full_text,
)
from arxiv_archive.indexing.page_index import build_page_index, build_page_index_from_parsed
from arxiv_archive.parsing.parser import PARSER_VERSION, parse_article
from arxiv_archive.parsing.structure import ParsedArticle, ParsedArticleElement

FULL_TEXT_FIXTURES = Path(__file__).parent / "fixtures" / "full_text"
PAGE_INDEX_FIXTURES = Path(__file__).parent / "fixtures" / "page_index"

SAFE_TITLES = st.from_regex(r"[A-Z][A-Za-z0-9 ]{2,24}", fullmatch=True).filter(
    lambda value: value.strip() == value and "  " not in value
)
BODY_TEXT = st.text(
    alphabet=st.characters(blacklist_categories=("Cs",), blacklist_characters="\r#"),
    min_size=1,
    max_size=80,
).filter(lambda value: value.strip() != "")


def _ingestion(paper_id: str, text: str, source_path: Path | None = None) -> FullTextIngestionResult:
    stripped = text.strip()
    heading_count = sum(1 for line in stripped.splitlines() if line.startswith("#"))
    quality = FullTextQualityReport(
        status="ok",
        char_count=len(stripped),
        line_count=len(stripped.splitlines()),
        heading_count=heading_count,
        non_heading_nonempty_line_count=sum(
            1 for line in stripped.splitlines() if line.strip() and not line.startswith("#")
        ),
        warnings=[],
        fallback_reason=None,
    )
    return FullTextIngestionResult(
        paper_id=paper_id,
        source_type="markdown",
        source_path=source_path or Path("inline/generated.md"),
        text=stripped,
        extraction_mode="structured_markdown",
        warnings=[],
        fallback_reason=None,
        quality=quality,
        provenance={"paper_id": paper_id, "source_path": str(source_path or Path("inline/generated.md"))},
    )


def _markdown_from_sections(root_title: str, child_sections: list[tuple[int, str, str]]) -> str:
    lines = [f"# {root_title}", "Root body text."]
    for level, title, body in child_sections:
        lines.extend(["", f"{'#' * level} {title}", body])
    return "\n".join(lines)


def _summary(document) -> dict[str, object]:
    return {
        "paper_id": document.paper_id,
        "node_count": len(document.nodes),
        "titles": [node.title for node in document.nodes],
        "node_ids": [node.id for node in document.nodes],
        "paths": [node.path for node in document.nodes],
        "root_children": document.root.children_ids,
        "next_ids": [node.next_id for node in document.nodes],
        "provenance": {
            "parser": document.provenance["parser"],
            "parse_fallback": document.provenance["parse_fallback"],
            "section_count": document.provenance["section_count"],
            "page_index_builder": document.provenance["page_index_builder"],
            "node_count": document.provenance["node_count"],
            "navigation_anchor_count": document.provenance["navigation_anchor_count"],
        },
    }


def test_real_article_fixture_matches_golden_page_index_summary() -> None:
    source = FullTextSource(
        paper_id="2605.12345",
        source_type="markdown",
        source_path=FULL_TEXT_FIXTURES / "structured_paper.md",
    )
    expected_summary = json.loads(
        (PAGE_INDEX_FIXTURES / "structured_paper_summary.json").read_text(encoding="utf-8")
    )

    document = build_page_index(ingest_full_text(source))

    assert _summary(document) == expected_summary
    assert document.validate_navigation() == []
    assert [anchor.node_id for anchor in document.navigation_anchors] == expected_summary["node_ids"]
    assert [anchor.path for anchor in document.navigation_anchors] == expected_summary["paths"]


@settings(max_examples=80)
@given(
    root_title=SAFE_TITLES,
    child_sections=st.lists(
        st.tuples(st.integers(min_value=2, max_value=4), SAFE_TITLES, BODY_TEXT),
        min_size=1,
        max_size=8,
        unique_by=lambda item: item[1],
    ),
)
def test_generated_heading_trees_preserve_navigation_invariants(
    root_title: str, child_sections: list[tuple[int, str, str]]
) -> None:
    article = parse_article(
        _ingestion("property.paper", _markdown_from_sections(root_title, child_sections))
    )
    document = build_page_index_from_parsed(article)

    assert document.validate_navigation() == []
    assert document.provenance["parse_fallback"] == "false"
    assert document.provenance["node_count"] == str(len(child_sections) + 1)
    assert [node.order for node in document.nodes] == list(range(len(document.nodes)))
    assert [node.id for node in document.walk_next()] == [node.id for node in document.nodes]
    assert [anchor.node_id for anchor in document.navigation_anchors] == [
        node.id for node in document.nodes
    ]
    assert len({node.id for node in document.nodes}) == len(document.nodes)
    for node in document.nodes:
        assert document.node_by_id(node.id) == node
        assert document.path_to(node.id) == node.path
        assert node.path[-1] == node.id
        if node.parent_id is None:
            assert node.path == [node.id]
        else:
            parent = document.node_by_id(node.parent_id)
            assert parent is not None
            assert node.id in parent.children_ids
            assert node.path == [*parent.path, node.id]


def test_duplicate_heading_titles_get_deterministic_ids_and_full_path_coverage() -> None:
    markdown = """
# Duplicate Study

## Method
First method section.

### Detail
Nested method detail.

## Method
Second method section.

### Detail
Nested detail for the second method.
"""

    document = build_page_index(_ingestion("duplicate.paper", markdown))

    assert [node.id for node in document.nodes] == [
        "duplicate.paper:root",
        "duplicate.paper:method",
        "duplicate.paper:detail",
        "duplicate.paper:method-2",
        "duplicate.paper:detail-2",
    ]
    assert document.validate_navigation() == []
    assert document.path_to("duplicate.paper:detail") == [
        "duplicate.paper:root",
        "duplicate.paper:method",
        "duplicate.paper:detail",
    ]
    assert document.path_to("duplicate.paper:detail-2") == [
        "duplicate.paper:root",
        "duplicate.paper:method-2",
        "duplicate.paper:detail-2",
    ]
    assert document.navigation_anchors[-1].path == document.path_to("duplicate.paper:detail-2")


def test_no_heading_input_falls_back_with_visible_parse_decision() -> None:
    document = build_page_index(
        _ingestion("fallback.paper", "This fixture has body text but no markdown headings.")
    )

    assert document.validate_navigation() == []
    assert document.validation_warnings == [
        "no markdown headings found; created fallback full-text section"
    ]
    assert document.provenance["parse_fallback"] == "true"
    assert document.provenance["fallback_reason"] == "no_headings"
    assert document.provenance["node_count"] == "2"
    assert [node.title for node in document.nodes] == ["Document", "Full Text"]
    assert document.root.children_ids == ["fallback.paper:full-text"]


def test_page_index_validation_reports_malformed_parsed_hierarchy_without_crashing() -> None:
    root = ParsedArticleElement(
        id="malformed.paper:root",
        paper_id="malformed.paper",
        kind="section",
        title="Root",
        level=1,
        order=0,
        text="",
        source_path=Path("inline/malformed.md"),
        path=["malformed.paper:root"],
        parent_id=None,
        provenance={"parser": PARSER_VERSION},
    )
    orphan = ParsedArticleElement(
        id="malformed.paper:orphan",
        paper_id="malformed.paper",
        kind="section",
        title="Orphan",
        level=2,
        order=1,
        text="orphaned body",
        source_path=Path("inline/malformed.md"),
        path=["malformed.paper:missing-parent", "malformed.paper:orphan"],
        parent_id="malformed.paper:missing-parent",
        provenance={"parser": PARSER_VERSION},
    )
    parsed = ParsedArticle(
        paper_id="malformed.paper",
        source_path=Path("inline/malformed.md"),
        elements=[root, orphan],
        validation_warnings=[],
        provenance={"parser": PARSER_VERSION, "parse_fallback": "false", "section_count": "2"},
    )

    document = build_page_index_from_parsed(parsed)

    assert document.validate_navigation() == [
        "node malformed.paper:orphan references missing parent malformed.paper:missing-parent"
    ]
    assert document.navigation_anchors[1].parent_id == "malformed.paper:missing-parent"
