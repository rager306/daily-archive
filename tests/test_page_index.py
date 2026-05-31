"""Contract tests for S02 PageIndex document navigation.

These tests define the PageIndex hierarchy and navigation contract before
implementation. They consume only S01 local full-text ingestion results.
"""

from __future__ import annotations

from pathlib import Path

from arxiv_archive.full_text import FullTextSource, ingest_full_text
from arxiv_archive.indexing.page_index import build_page_index_from_parsed
from arxiv_archive.page_index import build_page_index
from arxiv_archive.parsing.parser import parse_article

FULL_TEXT_FIXTURES = Path(__file__).parent / "fixtures" / "full_text"
PAGE_INDEX_FIXTURES = Path(__file__).parent / "fixtures" / "page_index"


def ingest_fixture(paper_id: str, source_path: Path, source_type: str = "markdown"):
    return ingest_full_text(
        FullTextSource(
            paper_id=paper_id,
            source_type=source_type,
            source_path=source_path,
        )
    )


def test_parser_boundary_emits_typed_elements_before_indexing() -> None:
    ingestion = ingest_fixture("2605.12345", FULL_TEXT_FIXTURES / "structured_paper.md")

    parsed = parse_article(ingestion)
    document = build_page_index_from_parsed(parsed)

    assert parsed.provenance["parser"] == "markdown_headings_v1"
    assert parsed.provenance["parse_fallback"] == "false"
    assert parsed.provenance["section_count"] == "5"
    assert [element.id for element in parsed.elements] == [node.id for node in document.nodes]
    assert parsed.elements[3].title == "Method"
    assert parsed.elements[3].path == ["2605.12345:root", "2605.12345:method"]
    assert parsed.elements[3].parent_id == "2605.12345:root"
    assert document.provenance["page_index_builder"] == "parsed_article_v1"
    assert document.provenance["node_count"] == "5"
    assert [anchor.node_id for anchor in document.navigation_anchors] == [
        "2605.12345:root",
        "2605.12345:abstract",
        "2605.12345:introduction",
        "2605.12345:method",
        "2605.12345:conclusion",
    ]


def test_builds_ordered_pageindex_tree_from_structured_markdown() -> None:
    ingestion = ingest_fixture("2605.12345", FULL_TEXT_FIXTURES / "structured_paper.md")

    document = build_page_index(ingestion)

    assert document.paper_id == "2605.12345"
    assert document.source_path == ingestion.source_path
    assert document.root.id == "2605.12345:root"
    assert document.root.title == "Graph-Guided Retrieval for Scientific Agents"
    assert document.root.level == 1
    assert document.root.parent_id is None
    assert document.root.path == ["2605.12345:root"]
    assert document.validation_warnings == []
    assert document.provenance["source_path"] == str(ingestion.source_path)
    assert document.provenance["parser"] == "markdown_headings_v1"

    titles = [node.title for node in document.nodes]
    assert titles == [
        "Graph-Guided Retrieval for Scientific Agents",
        "Abstract",
        "Introduction",
        "Method",
        "Conclusion",
    ]
    assert [node.order for node in document.nodes] == [0, 1, 2, 3, 4]
    assert [node.next_id for node in document.nodes] == [
        "2605.12345:abstract",
        "2605.12345:introduction",
        "2605.12345:method",
        "2605.12345:conclusion",
        None,
    ]
    assert document.root.children_ids == [
        "2605.12345:abstract",
        "2605.12345:introduction",
        "2605.12345:method",
        "2605.12345:conclusion",
    ]


def test_can_locate_sections_and_return_stable_paths() -> None:
    ingestion = ingest_fixture("2605.12345", FULL_TEXT_FIXTURES / "structured_paper.md")
    document = build_page_index(ingestion)

    method = document.find_by_title("Method")
    conclusion = document.find_by_title("conclusion")

    assert method is not None
    assert method.id == "2605.12345:method"
    assert method.parent_id == "2605.12345:root"
    assert method.path == ["2605.12345:root", "2605.12345:method"]
    assert "PageIndex construction" not in method.text
    assert "PageIndex from deterministic local markdown" in method.text

    assert conclusion is not None
    assert document.node_by_id(method.id) == method
    assert document.path_to(conclusion.id) == ["2605.12345:root", "2605.12345:conclusion"]
    assert [node.id for node in document.children_of(document.root.id)] == [
        "2605.12345:abstract",
        "2605.12345:introduction",
        "2605.12345:method",
        "2605.12345:conclusion",
    ]


def test_validate_navigation_reports_no_diagnostics_for_valid_fixture() -> None:
    ingestion = ingest_fixture("2605.12345", FULL_TEXT_FIXTURES / "structured_paper.md")
    document = build_page_index(ingestion)

    assert document.validate_navigation() == []


def test_validate_navigation_reports_broken_parent_child_next_and_path() -> None:
    ingestion = ingest_fixture("2605.12345", FULL_TEXT_FIXTURES / "structured_paper.md")
    document = build_page_index(ingestion)
    method = document.find_by_title("Method")
    assert method is not None

    method.parent_id = "2605.12345:missing-parent"
    method.path = ["2605.12345:root"]
    document.root.children_ids.append("2605.12345:missing-child")
    document.root.next_id = "2605.12345:method"

    assert document.validate_navigation() == [
        "child 2605.12345:method parent 2605.12345:missing-parent does not match 2605.12345:root",
        "node 2605.12345:root references missing child 2605.12345:missing-child",
        "node 2605.12345:method path does not end with its own id",
        "node 2605.12345:method references missing parent 2605.12345:missing-parent",
        "node 2605.12345:root next_id 2605.12345:method does not match 2605.12345:abstract",
    ]


def test_walk_next_returns_document_order() -> None:
    ingestion = ingest_fixture("2605.12345", FULL_TEXT_FIXTURES / "structured_paper.md")
    document = build_page_index(ingestion)

    assert [node.id for node in document.walk_next()] == [
        "2605.12345:root",
        "2605.12345:abstract",
        "2605.12345:introduction",
        "2605.12345:method",
        "2605.12345:conclusion",
    ]


def test_parser_boundary_reports_fallback_before_indexing() -> None:
    ingestion = ingest_fixture(
        "2605.noheadings",
        PAGE_INDEX_FIXTURES / "no_headings.txt",
        source_type="text",
    )

    parsed = parse_article(ingestion)
    document = build_page_index_from_parsed(parsed)

    assert parsed.validation_warnings == [
        "no markdown headings found; created fallback full-text section"
    ]
    assert parsed.provenance["parse_fallback"] == "true"
    assert parsed.provenance["fallback_reason"] == "no_headings"
    assert [element.title for element in parsed.elements] == ["Document", "Full Text"]
    assert document.validation_warnings == parsed.validation_warnings
    assert document.navigation_anchors[0].children_ids == ["2605.noheadings:full-text"]


def test_no_heading_source_creates_fallback_section_with_diagnostic() -> None:
    ingestion = ingest_fixture(
        "2605.noheadings",
        PAGE_INDEX_FIXTURES / "no_headings.txt",
        source_type="text",
    )

    document = build_page_index(ingestion)

    assert document.paper_id == "2605.noheadings"
    assert document.root.id == "2605.noheadings:root"
    assert document.root.title == "Document"
    assert document.root.children_ids == ["2605.noheadings:full-text"]
    assert document.validation_warnings == [
        "no markdown headings found; created fallback full-text section"
    ]
    assert document.provenance["fallback_reason"] == "no_headings"

    fallback = document.find_by_title("Full Text")
    assert fallback is not None
    assert fallback.id == "2605.noheadings:full-text"
    assert fallback.parent_id == "2605.noheadings:root"
    assert fallback.path == ["2605.noheadings:root", "2605.noheadings:full-text"]
    assert "paper-like fixture has no markdown headings" in fallback.text
