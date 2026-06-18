"""Contract tests for S03 SemanticChunk and EvidencePath traceability.

These tests define the chunk/evidence substrate before implementation. They
consume S01 ingestion and S02 PageIndex construction end to end over fixtures.
"""

from __future__ import annotations

from pathlib import Path

from research_graph.papers.semantic_chunks import (
    EvidencePath,
    SemanticChunk,
    build_evidence_path,
    build_evidence_paths,
    build_semantic_chunks,
    build_semantic_chunks_from_parsed,
    validate_evidence_path,
)
from research_graph.corpus.ingestion import FullTextSource, ingest_full_text
from research_graph.papers.indexing import (
    PageIndexDocument,
    build_page_index,
    build_page_index_from_parsed,
)
from research_graph.corpus.parsing.parser import parse_article

FULL_TEXT_FIXTURES = Path(__file__).parent / "fixtures" / "full_text"
PAGE_INDEX_FIXTURES = Path(__file__).parent / "fixtures" / "page_index"


def build_document(
    paper_id: str = "2605.12345",
    source_path: Path | None = None,
    source_type: str = "markdown",
) -> PageIndexDocument:
    path = source_path or FULL_TEXT_FIXTURES / "structured_paper.md"
    ingestion = ingest_full_text(
        FullTextSource(
            paper_id=paper_id,
            source_type=source_type,
            source_path=path,
        )
    )
    return build_page_index(ingestion)


def ingest_fixture(
    paper_id: str = "2605.12345",
    source_path: Path | None = None,
    source_type: str = "markdown",
):
    path = source_path or FULL_TEXT_FIXTURES / "structured_paper.md"
    return ingest_full_text(
        FullTextSource(
            paper_id=paper_id,
            source_type=source_type,
            source_path=path,
        )
    )


def test_builds_semantic_chunks_directly_from_parser_output() -> None:
    ingestion = ingest_fixture()
    parsed = parse_article(ingestion)

    chunks_from_parsed = build_semantic_chunks_from_parsed(parsed)
    chunks_from_index = build_semantic_chunks(build_page_index_from_parsed(parsed))

    assert chunks_from_parsed == chunks_from_index
    assert [chunk.page_index_node_id for chunk in chunks_from_parsed] == [
        "2605.12345:abstract",
        "2605.12345:introduction",
        "2605.12345:method",
        "2605.12345:conclusion",
    ]


def test_builds_evidence_paths_for_all_chunks() -> None:
    document = build_document()
    chunks = build_semantic_chunks(document)

    paths = build_evidence_paths(document, chunks)

    assert [path.semantic_chunk_id for path in paths] == [chunk.id for chunk in chunks]
    assert all(path.validation_warnings == [] for path in paths)


def test_builds_deterministic_semantic_chunks_for_pageindex_nodes() -> None:
    document = build_document()

    chunks = build_semantic_chunks(document)

    assert [chunk.id for chunk in chunks] == [
        "2605.12345:abstract:chunk-0001",
        "2605.12345:introduction:chunk-0001",
        "2605.12345:method:chunk-0001",
        "2605.12345:conclusion:chunk-0001",
    ]
    assert [chunk.order for chunk in chunks] == [0, 1, 2, 3]
    assert {chunk.chunking_strategy for chunk in chunks} == {"section_text_v1"}

    method = next(chunk for chunk in chunks if chunk.page_index_node_id == "2605.12345:method")
    assert method.paper_id == "2605.12345"
    assert method.text == "The agent builds a PageIndex from deterministic local markdown before any network or PDF extraction is attempted."
    assert method.char_start == 0
    assert method.char_end == len(method.text)
    assert method.validation_warnings == []
    assert method.provenance == {
        "paper_id": "2605.12345",
        "page_index_node_id": "2605.12345:method",
        "page_index_path": "2605.12345:root/2605.12345:method",
        "chunking_strategy": "section_text_v1",
        "source_path": str(document.source_path),
    }


def test_skips_empty_root_and_reports_empty_section_diagnostic() -> None:
    document = build_document()
    abstract = document.find_by_title("Abstract")
    assert abstract is not None
    abstract.text = "   "

    chunks = build_semantic_chunks(document)

    assert all(chunk.page_index_node_id != document.root.id for chunk in chunks)
    assert all(chunk.page_index_node_id != abstract.id for chunk in chunks)
    assert document.validation_warnings == [
        "PageIndexNode 2605.12345:abstract has empty text; no SemanticChunk emitted"
    ]


def test_fallback_section_produces_traceable_chunk() -> None:
    document = build_document(
        paper_id="2605.noheadings",
        source_path=PAGE_INDEX_FIXTURES / "no_headings.txt",
        source_type="text",
    )

    chunks = build_semantic_chunks(document)

    assert len(chunks) == 1
    fallback = chunks[0]
    assert fallback.id == "2605.noheadings:full-text:chunk-0001"
    assert fallback.page_index_node_id == "2605.noheadings:full-text"
    assert fallback.page_index_path == ["2605.noheadings:root", "2605.noheadings:full-text"]
    assert "paper-like fixture has no markdown headings" in fallback.text
    assert fallback.provenance["page_index_path"] == "2605.noheadings:root/2605.noheadings:full-text"


def test_builds_valid_evidence_path_from_chunk() -> None:
    document = build_document()
    chunks = build_semantic_chunks(document)
    method = next(chunk for chunk in chunks if chunk.page_index_node_id == "2605.12345:method")

    path = build_evidence_path(document, method)

    assert path == EvidencePath(
        paper_id="2605.12345",
        page_index_node_id="2605.12345:method",
        semantic_chunk_id="2605.12345:method:chunk-0001",
        node_path=["2605.12345:root", "2605.12345:method"],
        validation_warnings=[],
        provenance={
            "paper_id": "2605.12345",
            "page_index_node_id": "2605.12345:method",
            "semantic_chunk_id": "2605.12345:method:chunk-0001",
            "source_path": str(document.source_path),
        },
    )
    assert validate_evidence_path(path, document, chunks) == []


def test_evidence_path_validation_reports_missing_and_mismatched_links() -> None:
    document = build_document()
    chunks = build_semantic_chunks(document)
    valid_chunk = chunks[0]
    broken = EvidencePath(
        paper_id="different-paper",
        page_index_node_id="2605.12345:missing-node",
        semantic_chunk_id=valid_chunk.id,
        node_path=["2605.12345:root", "2605.12345:missing-node"],
        validation_warnings=[],
        provenance={},
    )

    assert validate_evidence_path(broken, document, chunks) == [
        "evidence path paper_id different-paper does not match document paper_id 2605.12345",
        "evidence path references missing PageIndexNode 2605.12345:missing-node",
        "SemanticChunk 2605.12345:abstract:chunk-0001 belongs to node 2605.12345:abstract, not 2605.12345:missing-node",
        "SemanticChunk 2605.12345:abstract:chunk-0001 paper_id 2605.12345 does not match evidence path paper_id different-paper",
    ]


def test_validation_reports_missing_semantic_chunk() -> None:
    document = build_document()
    chunks = build_semantic_chunks(document)
    missing = EvidencePath(
        paper_id="2605.12345",
        page_index_node_id="2605.12345:method",
        semantic_chunk_id="2605.12345:method:missing-chunk",
        node_path=["2605.12345:root", "2605.12345:method"],
        validation_warnings=[],
        provenance={},
    )

    assert validate_evidence_path(missing, document, chunks) == [
        "evidence path references missing SemanticChunk 2605.12345:method:missing-chunk"
    ]


def test_validation_reports_node_path_mismatch() -> None:
    document = build_document()
    chunks = build_semantic_chunks(document)
    method = next(chunk for chunk in chunks if chunk.page_index_node_id == "2605.12345:method")
    broken_path = EvidencePath(
        paper_id="2605.12345",
        page_index_node_id="2605.12345:method",
        semantic_chunk_id=method.id,
        node_path=["2605.12345:root", "2605.12345:abstract"],
        validation_warnings=[],
        provenance={},
    )

    assert validate_evidence_path(broken_path, document, chunks) == [
        "evidence path node_path 2605.12345:root/2605.12345:abstract does not match PageIndexNode path 2605.12345:root/2605.12345:method"
    ]


def test_fallback_chunk_builds_valid_evidence_path() -> None:
    document = build_document(
        paper_id="2605.noheadings",
        source_path=PAGE_INDEX_FIXTURES / "no_headings.txt",
        source_type="text",
    )
    chunks = build_semantic_chunks(document)

    path = build_evidence_path(document, chunks[0])

    assert path.page_index_node_id == "2605.noheadings:full-text"
    assert path.semantic_chunk_id == "2605.noheadings:full-text:chunk-0001"
    assert path.node_path == ["2605.noheadings:root", "2605.noheadings:full-text"]
    assert path.validation_warnings == []
    assert validate_evidence_path(path, document, chunks) == []


def test_semantic_chunk_model_is_stable_for_downstream_storage() -> None:
    chunk = SemanticChunk(
        id="2605.12345:method:chunk-0001",
        paper_id="2605.12345",
        page_index_node_id="2605.12345:method",
        page_index_path=["2605.12345:root", "2605.12345:method"],
        order=0,
        text="fixture chunk",
        char_start=0,
        char_end=13,
        chunking_strategy="section_text_v1",
        validation_warnings=[],
        provenance={"paper_id": "2605.12345"},
    )

    assert chunk.id == "2605.12345:method:chunk-0001"
    assert chunk.page_index_path == ["2605.12345:root", "2605.12345:method"]
