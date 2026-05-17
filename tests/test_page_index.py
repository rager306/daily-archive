"""Contract tests for S02 PageIndex document navigation.

These tests define the PageIndex hierarchy and navigation contract before
implementation. They consume only S01 local full-text ingestion results.
"""

from __future__ import annotations

from pathlib import Path

from arxiv_archive.page_index import build_page_index

from arxiv_archive.full_text import FullTextSource, ingest_full_text

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
    assert document.path_to(conclusion.id) == ["2605.12345:root", "2605.12345:conclusion"]


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
