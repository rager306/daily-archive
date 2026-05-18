"""Contract tests for the S09 read-only RLM workflow harness."""

from __future__ import annotations

from copy import deepcopy
from pathlib import Path

from arxiv_archive.evidence import (
    EvidencePath,
    SemanticChunk,
    build_evidence_path,
    build_semantic_chunks,
)
from arxiv_archive.full_text import FullTextSource, ingest_full_text
from arxiv_archive.page_index import PageIndexDocument, build_page_index
from arxiv_archive.rlm_workflow import run_document_workflow

FULL_TEXT_FIXTURES = Path(__file__).parent / "fixtures" / "full_text"


def build_document() -> PageIndexDocument:
    ingestion = ingest_full_text(
        FullTextSource(
            paper_id="2605.12345",
            source_type="markdown",
            source_path=FULL_TEXT_FIXTURES / "structured_paper.md",
        )
    )
    return build_page_index(ingestion)


def build_valid_inputs() -> tuple[PageIndexDocument, list[SemanticChunk], list[EvidencePath]]:
    document = build_document()
    chunks = build_semantic_chunks(document)
    evidence_paths = [build_evidence_path(document, chunk) for chunk in chunks]
    return document, chunks, evidence_paths


def test_valid_fixture_yields_ordered_text_safe_trajectory() -> None:
    document, chunks, evidence_paths = build_valid_inputs()

    result = run_document_workflow(document, chunks=chunks, evidence_paths=evidence_paths)

    assert result.status == "ok"
    assert result.navigation_valid is True
    assert result.boundary_valid is True
    assert result.context is not None
    assert result.context.node_ids == (
        "2605.12345:root",
        "2605.12345:abstract",
        "2605.12345:introduction",
        "2605.12345:method",
        "2605.12345:conclusion",
    )
    assert result.context.semantic_chunk_ids == tuple(chunk.id for chunk in chunks)
    assert result.context.evidence_path_ids == tuple(path.semantic_chunk_id for path in evidence_paths)
    assert result.context.counts == (
        ("nodes", 5),
        ("chunks", 4),
        ("evidence_paths", 4),
        ("diagnostics", 0),
    )
    assert [step.phase for step in result.trajectory] == [
        "validate_navigation",
        "walk_next",
        "children",
        "path",
        "children",
        "path",
        "children",
        "path",
        "children",
        "path",
        "children",
        "path",
        "chunks",
        "evidence",
        "draft",
    ]
    assert result.trajectory[1].path_node_ids == result.context.node_ids
    assert result.diagnostics == ()

    rendered = repr(result)
    assert "Graph-Guided Retrieval" not in rendered
    assert "The agent builds" not in rendered
    assert "PageIndex from deterministic local markdown" not in rendered


def test_invalid_navigation_blocks_downstream_phases() -> None:
    document, chunks, evidence_paths = build_valid_inputs()
    method = document.find_by_title("Method")
    assert method is not None
    method.parent_id = "2605.12345:missing-parent"

    result = run_document_workflow(document, chunks=chunks, evidence_paths=evidence_paths)

    assert result.status == "blocked"
    assert result.navigation_valid is False
    assert result.boundary_valid is False
    assert result.context is None
    assert [step.phase for step in result.trajectory] == ["validate_navigation"]
    assert result.diagnostics == (
        result.trajectory[0].diagnostics[0],
        result.trajectory[0].diagnostics[1],
    )
    assert [diagnostic.message for diagnostic in result.diagnostics] == [
        "child 2605.12345:method parent 2605.12345:missing-parent does not match 2605.12345:root",
        "node 2605.12345:method references missing parent 2605.12345:missing-parent",
    ]


def test_expected_missing_evidence_is_a_deterministic_diagnostic() -> None:
    document, chunks, _ = build_valid_inputs()

    result = run_document_workflow(document, chunks=chunks, evidence_paths=[])

    assert result.status == "warning"
    assert result.navigation_valid is True
    assert result.boundary_valid is False
    assert result.context is not None
    assert result.context.counts[-1] == ("diagnostics", 1)
    assert result.diagnostics[0].message == "no evidence paths provided"
    evidence_step = next(step for step in result.trajectory if step.phase == "evidence")
    assert evidence_step.status == "warning"
    assert evidence_step.counts == (("evidence_paths", 0),)


def test_invalid_chunk_and_evidence_references_return_id_only_diagnostics() -> None:
    document, chunks, evidence_paths = build_valid_inputs()
    broken_chunk = SemanticChunk(
        id="2605.12345:missing-node:chunk-0001",
        paper_id="2605.12345",
        page_index_node_id="2605.12345:missing-node",
        page_index_path=["2605.12345:root", "2605.12345:missing-node"],
        order=99,
        text="raw text must not appear in diagnostics",
        char_start=0,
        char_end=39,
        chunking_strategy="section_text_v1",
        validation_warnings=[],
        provenance={},
    )
    broken_evidence = EvidencePath(
        paper_id="2605.12345",
        page_index_node_id="2605.12345:method",
        semantic_chunk_id="2605.12345:method:missing-chunk",
        node_path=["2605.12345:root", "2605.12345:method"],
        validation_warnings=[],
        provenance={},
    )

    result = run_document_workflow(
        document,
        chunks=[*chunks, broken_chunk],
        evidence_paths=[*evidence_paths, broken_evidence],
    )

    assert result.status == "warning"
    assert result.boundary_valid is False
    assert [diagnostic.message for diagnostic in result.diagnostics] == [
        "SemanticChunk 2605.12345:missing-node:chunk-0001 references missing PageIndexNode 2605.12345:missing-node",
        "evidence path references missing SemanticChunk 2605.12345:method:missing-chunk",
    ]
    rendered = repr(result.diagnostics)
    assert "raw text must not appear" not in rendered


def test_harness_does_not_mutate_inputs() -> None:
    document, chunks, evidence_paths = build_valid_inputs()
    before_nodes = deepcopy(document.nodes)
    before_warnings = list(document.validation_warnings)
    before_chunks = deepcopy(chunks)
    before_evidence_paths = deepcopy(evidence_paths)

    run_document_workflow(document, chunks=chunks, evidence_paths=evidence_paths)

    assert document.nodes == before_nodes
    assert document.validation_warnings == before_warnings
    assert chunks == before_chunks
    assert evidence_paths == before_evidence_paths


def test_future_draft_and_boundary_output_are_hidden_from_repr() -> None:
    document, chunks, evidence_paths = build_valid_inputs()

    result = run_document_workflow(
        document,
        chunks=chunks,
        evidence_paths=evidence_paths,
        draft={"claim_text": "secret future claim text"},
        boundary_output={"prompt": "secret future prompt"},
    )

    rendered = repr(result)
    assert "secret future claim text" not in rendered
    assert "secret future prompt" not in rendered
