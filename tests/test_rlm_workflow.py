"""Contract tests for the S09 read-only RLM workflow harness."""

from __future__ import annotations

import ast
import copy
import json
from pathlib import Path
from typing import Any

import pytest

from research_graph.domain.semantic_chunks import EvidencePath, SemanticChunk
from research_graph.infrastructure.corpus.ingestion import FullTextSource, ingest_full_text
from research_graph.infrastructure.papers.indexing import PageIndexDocument, build_page_index
from research_graph.infrastructure.papers.semantic_chunks import (
    build_evidence_path,
    build_semantic_chunks,
)
from research_graph.workflows.rlm.workflow import (
    REDUCER_SCHEMA_VERSION,
    WorkflowResult,
    WorkflowTrajectoryStep,
    run_document_workflow,
)

_FIXTURE_STRUCTURE_PATH = (
    Path(__file__).parent / "fixtures" / "article_artifacts" / "basic_article_structure.json"
)
_FULL_TEXT_FIXTURES = Path(__file__).parent / "fixtures" / "full_text"
_FIXTURE_STRUCTURE = json.loads(_FIXTURE_STRUCTURE_PATH.read_text(encoding="utf-8"))
_RLM_WORKFLOW_MODULE = Path("src/research_graph/workflows/rlm/workflow.py")
_SAFETY_KEYS = {
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
}
_FORBIDDEN_IMPORT_ROOTS = {
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
_FORBIDDEN_RUNTIME_REFERENCES = {
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
            source_path=_FULL_TEXT_FIXTURES / "structured_paper.md",
        )
    )
    return build_page_index(ingestion)


def build_valid_inputs() -> tuple[PageIndexDocument, list[SemanticChunk], list[EvidencePath]]:
    document = build_document()
    chunks = build_semantic_chunks(document)
    evidence_paths = [build_evidence_path(document, chunk) for chunk in chunks]
    return document, chunks, evidence_paths


def _structure(paper_id: str = "m052-rlm-workflow-contract") -> dict[str, Any]:
    structure = copy.deepcopy(_FIXTURE_STRUCTURE)
    structure["paper_id"] = paper_id
    return structure


def _minimal_structure(paper_id: str = "m052-rlm-workflow-minimal") -> dict[str, Any]:
    structure = _structure(paper_id)
    root = structure["sections"][0]
    structure["sections"] = [root]
    structure["artifact_placeholders"] = []
    structure["structured_markers"] = []
    structure["scientific_markers"] = []
    structure["paragraphs"] = []
    structure["safe_spans"] = [
        span for span in structure["safe_spans"] if span["span_id"] == root["span_id"]
    ]
    return structure


def _run(
    structure: dict[str, Any] | None = None, *, run_id: str = "run-contract", max_steps: int = 16
) -> WorkflowResult:
    return run_document_workflow(
        _structure() if structure is None else structure,
        page_index={"pages": []},
        chunks=[],
        evidence_paths=[],
        run_id=run_id,
        max_steps=max_steps,
    )


def _assert_safety_block_all_false(block: dict[str, Any]) -> None:
    assert set(block) == _SAFETY_KEYS
    assert all(value is False for value in block.values())


def _scrub_generated(value: Any) -> Any:
    if isinstance(value, dict):
        return {
            key: _scrub_generated(child)
            for key, child in value.items()
            if key not in {"generated_at", "started_at", "completed_at"}
        }
    if isinstance(value, list):
        return [_scrub_generated(child) for child in value]
    return value


def test_valid_fixture_yields_ordered_text_safe_trajectory() -> None:
    result = _run(run_id="run-ordered")

    assert result.trajectory.schema_version == REDUCER_SCHEMA_VERSION
    assert len(result.trajectory.steps) == 8
    assert [step.step_type for step in result.trajectory.steps] == [
        "section_navigate",
        "section_navigate",
        "section_navigate",
        "span_visit",
        "span_visit",
        "span_visit",
        "helper_invoke",
        "helper_invoke",
    ]
    assert all(step.section_id is not None for step in result.trajectory.steps[:3])
    assert all(step.span_id is not None for step in result.trajectory.steps[3:6])
    assert tuple(step.work_id for step in result.trajectory.steps[6:]) == result.trajectory.work_ids
    assert "Local markdown" not in repr(result.to_sanitized_dict())


def test_helper_invocation_returns_review_only_reducer_summary() -> None:
    result = _run(run_id="run-helper-summary")

    assert result.aggregate_summary["total_unique_work_ids"] == 2
    assert result.aggregate_summary["work_ids"] == sorted(result.trajectory.work_ids)
    assert result.safety_audit["helper_output_is_review_only"] is True
    assert result.safety_audit["import_authority"] == "import is not authorized"


def test_minimal_navigation_skips_helper_phase() -> None:
    result = _run(_minimal_structure(), run_id="run-minimal")

    assert [step.step_type for step in result.trajectory.steps] == [
        "section_navigate",
        "span_visit",
    ]
    assert result.trajectory.work_ids == ()
    assert result.aggregate_summary["total_unique_work_ids"] == 0
    assert result.safety_audit["all_reducer_safety_defaults_false"] is True


def test_invalid_structure_type_fails_before_helper_phase() -> None:
    with pytest.raises(TypeError, match="structure must be a dict"):
        run_document_workflow([], {}, [], [], run_id="run-invalid-type")  # type: ignore[arg-type]


def test_invalid_run_id_and_bounds_are_deterministic_diagnostics() -> None:
    with pytest.raises(ValueError, match="run_id must be non-empty"):
        run_document_workflow(_structure(), {}, [], [], run_id="")
    with pytest.raises(ValueError, match="max_steps must be positive"):
        run_document_workflow(_structure(), {}, [], [], run_id="run-bounds", max_steps=0)
    with pytest.raises(ValueError, match="max_candidates must be positive"):
        run_document_workflow(_structure(), {}, [], [], run_id="run-bounds", max_candidates=0)


def test_max_steps_blocks_downstream_phases() -> None:
    result = _run(run_id="run-max-steps", max_steps=4)

    assert len(result.trajectory.steps) == 4
    assert [step.step_type for step in result.trajectory.steps] == [
        "section_navigate",
        "section_navigate",
        "section_navigate",
        "span_visit",
    ]
    assert result.trajectory.work_ids == ()


def test_workflow_step_rejects_ambiguous_navigation_identity() -> None:
    with pytest.raises(ValueError, match="exactly one"):
        WorkflowTrajectoryStep(
            step_type="section_navigate",
            section_id="paper:section:intro",
            span_id="paper:span:intro",
            run_id="run-bad-step",
        )


def test_harness_does_not_mutate_inputs() -> None:
    structure = _structure("m052-no-mutation")
    before = copy.deepcopy(structure)

    _run(structure, run_id="run-no-mutation")

    assert structure == before


def test_determinism_byte_identical_after_scrubbing_generated_timestamps() -> None:
    structure = _structure("m052-deterministic-contract")
    first = _run(structure, run_id="run-deterministic")
    second = _run(copy.deepcopy(structure), run_id="run-deterministic")

    first_json = json.dumps(_scrub_generated(first.to_sanitized_dict()), sort_keys=True)
    second_json = json.dumps(_scrub_generated(second.to_sanitized_dict()), sort_keys=True)

    assert first_json == second_json


def test_all_5_safety_defaults_stay_false() -> None:
    result = _run(run_id="run-safety")

    _assert_safety_block_all_false(result.trajectory.aggregate_safety_defaults)
    _assert_safety_block_all_false(result.safety_audit["aggregate_safety_defaults"])
    _assert_safety_block_all_false(result.safety_audit["reducer_safety_defaults"])
    assert result.safety_audit["all_step_safety_defaults_false"] is True
    assert result.safety_audit["all_reducer_safety_defaults_false"] is True
    for step in result.trajectory.steps:
        _assert_safety_block_all_false(step.safety_defaults)


def test_rlm_workflow_module_does_not_reference_forbidden_runtime_storage_io_or_network() -> None:
    source = _RLM_WORKFLOW_MODULE.read_text(encoding="utf-8")
    tree = ast.parse(source)
    violations: list[str] = []

    imported_roots: set[str] = set()
    runtime_references: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            imported_roots.update(alias.name.split(".")[0] for alias in node.names)
        elif isinstance(node, ast.ImportFrom) and node.module:
            imported_roots.add(node.module.split(".")[0])
        elif isinstance(node, ast.Name):
            runtime_references.add(node.id)
        elif isinstance(node, ast.Attribute):
            runtime_references.add(node.attr)

    forbidden_imports = sorted(imported_roots & _FORBIDDEN_IMPORT_ROOTS)
    forbidden_refs = sorted(runtime_references & _FORBIDDEN_RUNTIME_REFERENCES)
    if forbidden_imports:
        violations.append(f"{_RLM_WORKFLOW_MODULE} forbidden imports: {forbidden_imports}")
    if forbidden_refs:
        violations.append(f"{_RLM_WORKFLOW_MODULE} forbidden runtime refs: {forbidden_refs}")

    assert violations == []


def test_result_trajectory_and_diagnostics_do_not_expose_network_or_graph_writes() -> None:
    result = _run(run_id="run-text-safe")
    serialized = json.dumps(result.to_sanitized_dict(), sort_keys=True)
    forbidden_loopback_hostname = "local" + "host"

    assert forbidden_loopback_hostname not in serialized
    assert "127.0.0.1" not in serialized
    assert 'graphdb_written": true' not in serialized
    assert 'ladybugdb_written": true' not in serialized
    assert 'production_import_attempted": true' not in serialized
