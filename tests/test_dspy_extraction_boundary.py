"""Fixture-backed S08 tests for the deterministic DSPy extraction boundary.

These tests pin the S08 boundary to the existing S04 ExtractionPatch contract and
S07 ID-only evaluation metrics. They intentionally stay local-only: no DSPy
runtime, optimizers, live clients, embeddings, network calls, or storage writes.
"""

from __future__ import annotations

import ast
from dataclasses import replace
from pathlib import Path
from typing import Any

import pytest

from arxiv_archive.dspy_extraction import BaselineDspyExtractionModule, DspyExtractionInput
from arxiv_archive.ladybug_client import evidence_path_id
from tests.test_ladybug_scientific_kg import build_fixture_payload

RAW_FIXTURE_CLAIM_TEXT = "Local markdown is enough to build a deterministic PageIndex."
S08_FILES = (
    Path("src/arxiv_archive/dspy_extraction.py"),
    Path("tests/test_dspy_extraction_boundary.py"),
)
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
    "ladybug",
    "Database",
    "Connection",
    "execute",
    "upsert_scientific_kg",
    "init_db",
}
ALLOWED_STRING_LITERAL_FRAGMENTS = FORBIDDEN_IMPORT_ROOTS | FORBIDDEN_RUNTIME_REFERENCES | {
    "DSPy",
    "optimizer",
    "optimizers",
    "live clients",
    "storage writes",
}


def _safe_text(value: Any) -> str:
    return repr(value)


def _fixture_patch_contract() -> tuple[Any, frozenset[str]]:
    _document, _chunks, evidence_paths, patch = build_fixture_payload()
    return patch, frozenset(evidence_path_id(path) for path in evidence_paths)


def test_fixture_baseline_forward_returns_patch_and_metric_boundary_fields() -> None:
    """A deterministic baseline callable should expose S04/S07 gates through S08."""
    patch, expected_evidence_ids = _fixture_patch_contract()
    module = BaselineDspyExtractionModule(lambda boundary_input: patch)

    output = module.forward(
        DspyExtractionInput(
            paper_id=patch.paper_id,
            expected_evidence_path_ids=expected_evidence_ids,
            baseline_context={"fixture": "s04-s07-local"},
        )
    )

    assert output.patch is patch
    assert output.boundary_version == "dspy_extraction_boundary.v1"
    assert output.schema_valid is True
    assert output.schema_diagnostics == []
    assert output.schema_diagnostic_count == 0
    assert output.groundedness_valid is True
    assert output.groundedness_diagnostics == {
        "claim_count": 1,
        "entity_count": 1,
        "relation_count": 1,
        "evidence_backed_claim_count": 1,
        "evidence_backed_entity_count": 1,
        "evidence_backed_relation_count": 1,
        "derived_evidence_path_ids": sorted(expected_evidence_ids),
        "missing_expected_evidence_path_ids": [],
        "unexpected_evidence_path_ids": [],
        "missing_evidence_path_draft_ids": [],
        "status": "valid",
    }
    assert output.boundary_valid is True
    assert output.boundary_diagnostics == []
    assert output.optimizer_enabled is False
    assert output.optimizer_name is None
    assert output.optimizer_rejection_reason is None


def test_fail_closed_invalid_schema_and_wrong_expected_evidence_are_id_count_based() -> None:
    """Schema and groundedness failures should remain inspectable without body text."""
    patch, expected_evidence_ids = _fixture_patch_contract()
    invalid_patch = replace(
        patch,
        claims=[replace(patch.claims[0], confidence=1.7)],
    )
    module = BaselineDspyExtractionModule(lambda boundary_input: invalid_patch)

    output = module.forward(
        DspyExtractionInput(
            paper_id=patch.paper_id,
            expected_evidence_path_ids=frozenset({*expected_evidence_ids, "evidence:missing:expected"}),
        )
    )

    assert output.patch is invalid_patch
    assert output.schema_valid is False
    assert output.schema_diagnostic_count == 1
    assert output.schema_diagnostics == [
        "Claim claim:2605.12345:method:chunk-0001:local-markdown-pageindex confidence 1.7 is outside [0.0, 1.0]"
    ]
    assert output.groundedness_valid is False
    assert output.groundedness_diagnostics["status"] == "invalid"
    assert output.groundedness_diagnostics["missing_expected_evidence_path_ids"] == [
        "evidence:missing:expected"
    ]
    assert output.groundedness_diagnostics["unexpected_evidence_path_ids"] == []
    assert output.groundedness_diagnostics["derived_evidence_path_ids"] == sorted(expected_evidence_ids)
    assert output.boundary_valid is False
    assert RAW_FIXTURE_CLAIM_TEXT not in _safe_text(output.schema_diagnostics)
    assert RAW_FIXTURE_CLAIM_TEXT not in _safe_text(output.groundedness_diagnostics)
    assert RAW_FIXTURE_CLAIM_TEXT not in _safe_text(output.boundary_diagnostics)


def test_missing_draft_evidence_is_reported_by_draft_id_not_claim_text() -> None:
    """Malformed patches with missing evidence should expose draft IDs only."""
    patch, expected_evidence_ids = _fixture_patch_contract()
    missing_evidence_patch = replace(
        patch,
        claims=[replace(patch.claims[0], evidence_path=None)],
    )
    module = BaselineDspyExtractionModule(lambda boundary_input: missing_evidence_patch)

    output = module.forward(
        DspyExtractionInput(
            paper_id=patch.paper_id,
            expected_evidence_path_ids=expected_evidence_ids,
        )
    )

    assert output.schema_valid is False
    assert output.groundedness_valid is False
    assert output.groundedness_diagnostics["missing_evidence_path_draft_ids"] == [
        patch.claims[0].id
    ]
    assert output.groundedness_diagnostics["missing_expected_evidence_path_ids"] == []
    assert output.groundedness_diagnostics["derived_evidence_path_ids"] == sorted(expected_evidence_ids)
    assert RAW_FIXTURE_CLAIM_TEXT not in _safe_text(output)
    assert output.patch is missing_evidence_patch
    assert RAW_FIXTURE_CLAIM_TEXT in output.patch.claims[0].text


@pytest.mark.parametrize(
    ("kwargs", "call_kwargs"),
    [
        ({"optimizer_name": "MIPRO"}, {}),
        ({"optimizer_config": {"name": "GEPA"}}, {}),
        ({}, {"optimizer_name": "BootstrapFewShot"}),
        ({}, {"optimizer_config": {"name": "dspy.teleprompt.MIPRO"}}),
    ],
)
def test_optimizer_requests_are_rejected_without_enabling_runtime(
    kwargs: dict[str, Any], call_kwargs: dict[str, Any]
) -> None:
    """Optimizer names/configuration must fail closed rather than enable runtime."""
    patch, _expected_evidence_ids = _fixture_patch_contract()

    if kwargs:
        with pytest.raises(ValueError, match="optimizer runtime is disabled") as exc_info:
            BaselineDspyExtractionModule(lambda boundary_input: patch, **kwargs)
        assert "dspy_extraction_boundary.v1" in str(exc_info.value)
        return

    module = BaselineDspyExtractionModule(lambda boundary_input: patch)
    assert module.optimizer_enabled is False
    with pytest.raises(ValueError, match="optimizer runtime is disabled") as exc_info:
        module.forward(DspyExtractionInput(paper_id=patch.paper_id), **call_kwargs)
    assert "dspy_extraction_boundary.v1" in str(exc_info.value)
    assert module.optimizer_enabled is False


def test_boundary_output_repr_and_diagnostics_do_not_expose_fixture_claim_text() -> None:
    """The patch may carry claim text, but the boundary repr/diagnostics must not."""
    patch, expected_evidence_ids = _fixture_patch_contract()
    module = BaselineDspyExtractionModule(lambda boundary_input: patch)

    output = module.forward(
        DspyExtractionInput(
            paper_id=patch.paper_id,
            expected_evidence_path_ids=frozenset({*expected_evidence_ids, "evidence:missing:expected"}),
        )
    )

    assert output.patch is patch
    assert RAW_FIXTURE_CLAIM_TEXT in output.patch.claims[0].text
    assert RAW_FIXTURE_CLAIM_TEXT not in repr(output)
    assert RAW_FIXTURE_CLAIM_TEXT not in _safe_text(output.schema_diagnostics)
    assert RAW_FIXTURE_CLAIM_TEXT not in _safe_text(output.groundedness_diagnostics)
    assert RAW_FIXTURE_CLAIM_TEXT not in _safe_text(output.boundary_diagnostics)


def _module_static_scope(path: Path) -> tuple[list[str], list[str], list[str]]:
    source = path.read_text(encoding="utf-8")
    tree = ast.parse(source, filename=str(path))
    imported_roots: list[str] = []
    runtime_refs: list[str] = []
    string_violations: list[str] = []

    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            imported_roots.extend(alias.name.split(".")[0] for alias in node.names)
        elif isinstance(node, ast.ImportFrom) and node.module is not None:
            imported_roots.append(node.module.split(".")[0])
        elif isinstance(node, ast.Name):
            runtime_refs.append(node.id)
        elif isinstance(node, ast.Attribute):
            runtime_refs.append(node.attr)
        elif isinstance(node, ast.Constant) and isinstance(node.value, str):
            for forbidden in FORBIDDEN_RUNTIME_REFERENCES:
                if forbidden in node.value and forbidden not in ALLOWED_STRING_LITERAL_FRAGMENTS:
                    string_violations.append(f"{path}:{forbidden}")

    return imported_roots, runtime_refs, string_violations


def test_s08_files_do_not_reference_forbidden_runtime_or_storage_scopes() -> None:
    """S08 must remain local-only and free of forbidden optimizer/runtime/storage APIs."""
    violations: list[str] = []

    for path in S08_FILES:
        imports, refs, string_violations = _module_static_scope(path)
        bad_imports = sorted(set(imports) & FORBIDDEN_IMPORT_ROOTS)
        bad_refs = sorted(set(refs) & FORBIDDEN_RUNTIME_REFERENCES)
        if bad_imports:
            violations.append(f"{path} forbidden imports: {bad_imports}")
        if bad_refs:
            violations.append(f"{path} forbidden runtime refs: {bad_refs}")
        violations.extend(string_violations)

    assert violations == []
