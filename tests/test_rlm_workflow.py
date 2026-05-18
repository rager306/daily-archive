"""Contract tests for the S09 read-only RLM workflow harness."""

from __future__ import annotations

import ast
from copy import deepcopy
from dataclasses import replace
from pathlib import Path
from typing import Any

import pytest

from arxiv_archive.dspy_extraction import BaselineDspyExtractionModule, DspyExtractionInput
from arxiv_archive.evidence import (
    EvidencePath,
    SemanticChunk,
    build_evidence_path,
    build_semantic_chunks,
)
from arxiv_archive.full_text import FullTextSource, ingest_full_text
from arxiv_archive.ladybug_client import evidence_path_id
from arxiv_archive.page_index import PageIndexDocument, build_page_index
from arxiv_archive.rlm_workflow import run_document_workflow
from arxiv_archive.scientific_extraction import (
    Claim,
    ExtractionPatch,
    ScientificEntity,
    ScientificRelation,
)

FULL_TEXT_FIXTURES = Path(__file__).parent / "fixtures" / "full_text"
RLM_WORKFLOW_MODULE = Path("src/arxiv_archive/rlm_workflow.py")
SCHEMA_VERSION = "scientific_extraction.v1"
EXTRACTOR_VERSION = "fixture-extractor.v1"
RAW_FIXTURE_BODY_TEXT = (
    "The agent builds a PageIndex from deterministic local markdown before any network or "
    "PDF extraction is attempted."
)
RAW_FIXTURE_CLAIM_TEXT = "Local markdown is enough to build a deterministic PageIndex."
FORBIDDEN_IMPORT_ROOTS = {
    "dspy",
    "socket",
    "httpx",
    "requests",
    "urllib",
    "openai",
    "anthropic",
    "cohere",
    "sentence_transformers",
    "transformers",
    "subprocess",
    "os",
    "pathlib",
    "sqlite3",
}
FORBIDDEN_RUNTIME_REFERENCES = {
    "dspy",
    "teleprompt",
    "MIPRO",
    "MIPROv2",
    "GEPA",
    "BootstrapFewShot",
    "BootstrapFewShotWithRandomSearch",
    "socket",
    "create_connection",
    "HTTPConnection",
    "HTTPSConnection",
    "httpx",
    "requests",
    "urlopen",
    "OpenAI",
    "Anthropic",
    "Cohere",
    "SentenceTransformer",
    "Embedding",
    "embeddings",
    "LadybugDB",
    "Database",
    "Connection",
    "connect",
    "execute",
    "executemany",
    "commit",
    "upsert_scientific_kg",
    "init_db",
    "subprocess",
    "Popen",
    "run",
    "call",
    "check_call",
    "check_output",
    "system",
    "popen",
    "Path",
    "open",
    "write",
    "writelines",
    "write_text",
    "write_bytes",
    "dump",
    "dumps",
}


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


def build_fixture_patch(evidence: EvidencePath) -> ExtractionPatch:
    claim = Claim(
        id="claim:2605.12345:method:chunk-0001:local-markdown-pageindex",
        paper_id="2605.12345",
        text=RAW_FIXTURE_CLAIM_TEXT,
        claim_type="method",
        confidence=0.91,
        evidence_path=evidence,
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "fixture"},
    )
    entity = ScientificEntity(
        id="entity:2605.12345:pageindex",
        paper_id="2605.12345",
        label="PageIndex",
        entity_type="method",
        confidence=0.88,
        evidence_path=evidence,
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "fixture"},
    )
    relation = ScientificRelation(
        id="relation:2605.12345:claim-local-markdown-pageindex:entity-pageindex:supports",
        paper_id="2605.12345",
        relation_type="supports",
        source_id=claim.id,
        target_id=entity.id,
        confidence=0.84,
        evidence_path=evidence,
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "fixture"},
    )
    return ExtractionPatch(
        paper_id="2605.12345",
        claims=[claim],
        entities=[entity],
        relations=[relation],
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "fixture"},
    )


def method_evidence(chunks: list[SemanticChunk], evidence_paths: list[EvidencePath]) -> EvidencePath:
    method_chunk = next(chunk for chunk in chunks if chunk.page_index_node_id == "2605.12345:method")
    return next(path for path in evidence_paths if path.semantic_chunk_id == method_chunk.id)


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
    assert result.context.evidence_path_ids == tuple(evidence_path_id(path) for path in evidence_paths)
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
    assert result.trajectory[-1].counts[-1] == ("extractor_calls", 0)
    assert result.diagnostics == ()

    rendered = repr(result)
    assert "Graph-Guided Retrieval" not in rendered
    assert "The agent builds" not in rendered
    assert "PageIndex from deterministic local markdown" not in rendered


def test_valid_extraction_boundary_is_called_once_and_returns_hidden_patch() -> None:
    document, chunks, evidence_paths = build_valid_inputs()
    evidence = method_evidence(chunks, evidence_paths)
    patch = build_fixture_patch(evidence)
    calls: list[DspyExtractionInput] = []

    def extractor(boundary_input: DspyExtractionInput) -> ExtractionPatch:
        calls.append(boundary_input)
        return patch

    result = run_document_workflow(
        document,
        chunks=chunks,
        evidence_paths=[evidence],
        extractor=BaselineDspyExtractionModule(extractor),
    )

    assert len(calls) == 1
    assert calls[0].paper_id == "2605.12345"
    assert calls[0].expected_evidence_path_ids == frozenset({evidence_path_id(evidence)})
    assert calls[0].baseline_context == {
        "paper_id": "2605.12345",
        "root_node_id": "2605.12345:root",
        "page_index_node_ids": result.context.node_ids if result.context else (),
        "semantic_chunk_ids": tuple(chunk.id for chunk in chunks),
        "evidence_path_ids": (evidence_path_id(evidence),),
        "counts": {"nodes": 5, "chunks": 4, "evidence_paths": 1, "diagnostics": 0},
        "route_status": "navigation_validated",
    }
    assert result.status == "ok"
    assert result.boundary_valid is True
    assert result.draft is patch
    assert result.boundary_output is not None
    assert result.boundary_output.patch is patch
    draft_step = result.trajectory[-1]
    assert draft_step.phase == "draft"
    assert draft_step.status == "ok"
    assert draft_step.schema_valid is True
    assert draft_step.groundedness_valid is True
    assert draft_step.optimizer_enabled is False
    assert ("extractor_calls", 1) in draft_step.counts
    assert RAW_FIXTURE_CLAIM_TEXT in result.draft.claims[0].text
    assert RAW_FIXTURE_CLAIM_TEXT not in repr(result)
    assert RAW_FIXTURE_CLAIM_TEXT not in repr(result.trajectory)
    assert RAW_FIXTURE_CLAIM_TEXT not in repr(result.diagnostics)


def test_invalid_navigation_blocks_downstream_phases_and_does_not_call_extractor() -> None:
    document, chunks, evidence_paths = build_valid_inputs()
    method = document.find_by_title("Method")
    assert method is not None
    method.parent_id = "2605.12345:missing-parent"
    calls = 0

    def extractor(boundary_input: DspyExtractionInput) -> ExtractionPatch:
        nonlocal calls
        calls += 1
        return build_fixture_patch(evidence_paths[0])

    result = run_document_workflow(
        document,
        chunks=chunks,
        evidence_paths=evidence_paths,
        extractor=BaselineDspyExtractionModule(extractor),
    )

    assert calls == 0
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


def test_boundary_exception_and_malformed_return_fail_closed_with_safe_diagnostics() -> None:
    document, chunks, evidence_paths = build_valid_inputs()
    evidence = method_evidence(chunks, evidence_paths)

    exception_result = run_document_workflow(
        document,
        chunks=chunks,
        evidence_paths=[evidence],
        extractor=BaselineDspyExtractionModule(
            lambda boundary_input: (_ for _ in ()).throw(RuntimeError("raw failure text"))
        ),
    )
    assert exception_result.status == "warning"
    assert exception_result.boundary_output is not None
    assert exception_result.boundary_output.boundary_diagnostics == ["extractor_error:RuntimeError"]
    assert [diagnostic.message for diagnostic in exception_result.diagnostics] == [
        "schema_invalid",
        "groundedness_invalid",
        "boundary_invalid",
        "extractor_error:RuntimeError",
    ]
    assert "raw failure text" not in repr(exception_result)

    malformed_result = run_document_workflow(
        document,
        chunks=chunks,
        evidence_paths=[evidence],
        extractor=BaselineDspyExtractionModule(lambda boundary_input: {"claim": RAW_FIXTURE_CLAIM_TEXT}),
    )
    assert malformed_result.status == "warning"
    assert malformed_result.boundary_output is not None
    assert malformed_result.boundary_output.boundary_diagnostics == ["invalid_extractor_output:dict"]
    assert RAW_FIXTURE_CLAIM_TEXT not in repr(malformed_result)


def test_wrong_expected_evidence_and_missing_draft_evidence_fail_closed() -> None:
    document, chunks, evidence_paths = build_valid_inputs()
    evidence = method_evidence(chunks, evidence_paths)
    abstract_evidence = next(
        path for path in evidence_paths if path.page_index_node_id == "2605.12345:abstract"
    )
    wrong_evidence_patch = build_fixture_patch(abstract_evidence)

    wrong_result = run_document_workflow(
        document,
        chunks=chunks,
        evidence_paths=[evidence],
        extractor=BaselineDspyExtractionModule(lambda boundary_input: wrong_evidence_patch),
    )

    assert wrong_result.status == "warning"
    assert wrong_result.boundary_valid is False
    assert wrong_result.boundary_output is not None
    assert wrong_result.boundary_output.schema_valid is True
    assert wrong_result.boundary_output.groundedness_valid is False
    assert wrong_result.boundary_output.groundedness_diagnostics[
        "missing_expected_evidence_path_ids"
    ] == [evidence_path_id(evidence)]
    assert wrong_result.boundary_output.groundedness_diagnostics["unexpected_evidence_path_ids"] == [
        evidence_path_id(abstract_evidence)
    ]

    missing_evidence_patch = replace(
        build_fixture_patch(evidence),
        claims=[replace(build_fixture_patch(evidence).claims[0], evidence_path=None)],
    )
    missing_result = run_document_workflow(
        document,
        chunks=chunks,
        evidence_paths=[evidence],
        extractor=BaselineDspyExtractionModule(lambda boundary_input: missing_evidence_patch),
    )

    assert missing_result.status == "warning"
    assert missing_result.boundary_valid is False
    assert missing_result.boundary_output is not None
    assert missing_result.boundary_output.schema_valid is False
    assert missing_result.boundary_output.groundedness_valid is False
    assert missing_result.boundary_output.groundedness_diagnostics[
        "missing_evidence_path_draft_ids"
    ] == [missing_evidence_patch.claims[0].id]
    assert RAW_FIXTURE_CLAIM_TEXT not in repr(missing_result)


def test_optimizer_request_rejection_is_not_suppressed() -> None:
    document, chunks, evidence_paths = build_valid_inputs()
    evidence = method_evidence(chunks, evidence_paths)
    patch = build_fixture_patch(evidence)

    with pytest.raises(ValueError, match="optimizer runtime is disabled"):
        run_document_workflow(
            document,
            chunks=chunks,
            evidence_paths=[evidence],
            extractor=BaselineDspyExtractionModule(lambda boundary_input: patch),
            optimizer_config={"name": "MIPRO"},
        )


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


def test_draft_and_boundary_output_are_hidden_from_repr() -> None:
    document, chunks, evidence_paths = build_valid_inputs()
    evidence = method_evidence(chunks, evidence_paths)
    patch = build_fixture_patch(evidence)

    result = run_document_workflow(
        document,
        chunks=chunks,
        evidence_paths=[evidence],
        extractor=BaselineDspyExtractionModule(lambda boundary_input: patch),
    )

    rendered = repr(result)
    assert result.draft is patch
    assert result.boundary_output is not None
    assert result.boundary_output.patch is patch
    assert RAW_FIXTURE_CLAIM_TEXT not in rendered
    assert RAW_FIXTURE_CLAIM_TEXT not in repr(result.boundary_output)
    assert RAW_FIXTURE_CLAIM_TEXT not in repr(result.trajectory)


def _rlm_workflow_static_scope(path: Path) -> tuple[list[str], list[str]]:
    source = path.read_text(encoding="utf-8")
    tree = ast.parse(source, filename=str(path))
    imported_roots: list[str] = []
    runtime_refs: list[str] = []

    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            imported_roots.extend(alias.name.split(".")[0] for alias in node.names)
        elif isinstance(node, ast.ImportFrom) and node.module is not None:
            imported_roots.append(node.module.split(".")[0])
        elif isinstance(node, ast.Name):
            runtime_refs.append(node.id)
        elif isinstance(node, ast.Attribute):
            runtime_refs.append(node.attr)

    return imported_roots, runtime_refs


def test_rlm_workflow_module_does_not_reference_forbidden_runtime_storage_or_io() -> None:
    imports, refs = _rlm_workflow_static_scope(RLM_WORKFLOW_MODULE)
    violations: list[str] = []
    bad_imports = sorted(set(imports) & FORBIDDEN_IMPORT_ROOTS)
    bad_refs = sorted(set(refs) & FORBIDDEN_RUNTIME_REFERENCES)

    if bad_imports:
        violations.append(f"{RLM_WORKFLOW_MODULE} forbidden imports: {bad_imports}")
    if bad_refs:
        violations.append(f"{RLM_WORKFLOW_MODULE} forbidden runtime refs: {bad_refs}")

    assert violations == []


def test_result_trajectory_and_diagnostics_do_not_expose_body_or_claim_text() -> None:
    document, chunks, evidence_paths = build_valid_inputs()
    evidence = method_evidence(chunks, evidence_paths)
    patch = build_fixture_patch(evidence)

    result = run_document_workflow(
        document,
        chunks=chunks,
        evidence_paths=[evidence],
        extractor=BaselineDspyExtractionModule(lambda boundary_input: patch),
    )

    assert RAW_FIXTURE_BODY_TEXT in " ".join(chunk.text for chunk in chunks)
    assert RAW_FIXTURE_CLAIM_TEXT in result.draft.claims[0].text if result.draft else False
    assert result.boundary_output is not None
    safe_surfaces = {
        "result_repr": repr(result),
        "context_repr": repr(result.context),
        "trajectory_repr": repr(result.trajectory),
        "diagnostics_repr": repr(result.diagnostics),
        "boundary_output_repr": repr(result.boundary_output),
        "schema_diagnostics_repr": repr(result.boundary_output.schema_diagnostics),
        "groundedness_diagnostics_repr": repr(result.boundary_output.groundedness_diagnostics),
        "boundary_diagnostics_repr": repr(result.boundary_output.boundary_diagnostics),
    }

    leaks = [
        surface
        for surface, rendered in safe_surfaces.items()
        if RAW_FIXTURE_BODY_TEXT in rendered or RAW_FIXTURE_CLAIM_TEXT in rendered
    ]
    assert leaks == []
