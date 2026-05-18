"""Contract tests for the deterministic DSPy extraction boundary."""

from __future__ import annotations

from dataclasses import replace
from typing import cast

import pytest

from arxiv_archive.dspy_extraction import (
    BaselineDspyExtractionModule,
    DspyExtractionInput,
    dspy_extraction_signature_spec,
)
from arxiv_archive.ladybug_client import evidence_path_id
from arxiv_archive.scientific_extraction import ExtractionPatch
from tests.test_scientific_extraction_contracts import method_evidence_path, sample_patch


def test_boundary_wraps_extraction_patch_and_reports_metric_gates() -> None:
    patch = sample_patch()
    expected_ids = frozenset({evidence_path_id(method_evidence_path())})
    module = BaselineDspyExtractionModule(lambda boundary_input: patch)

    output = module.forward(
        DspyExtractionInput(
            paper_id=patch.paper_id,
            expected_evidence_path_ids=expected_ids,
        )
    )

    assert output.patch is patch
    assert output.boundary_version == "dspy_extraction_boundary.v1"
    assert output.optimizer_enabled is False
    assert output.optimizer_name is None
    assert output.optimizer_rejection_reason is None
    assert output.schema_valid is True
    assert output.schema_diagnostic_count == 0
    assert output.schema_diagnostics == []
    assert output.groundedness_valid is True
    assert output.groundedness_diagnostics["status"] == "valid"
    assert output.boundary_valid is True
    assert output.boundary_diagnostics == []


def test_boundary_rejects_optimizer_configuration_at_construction_and_invocation() -> None:
    patch = sample_patch()

    with pytest.raises(ValueError, match="optimizer runtime is disabled"):
        BaselineDspyExtractionModule(lambda boundary_input: patch, optimizer_name="bootstrap")

    module = BaselineDspyExtractionModule(lambda boundary_input: patch)
    with pytest.raises(ValueError, match="optimizer runtime is disabled"):
        module.forward(
            DspyExtractionInput(paper_id=patch.paper_id),
            optimizer_config={"name": "bootstrap"},
        )

    with pytest.raises(ValueError, match="optimizer runtime is disabled"):
        module.forward(
            DspyExtractionInput(paper_id=patch.paper_id, optimizer_config={"name": "bootstrap"})
        )


def test_boundary_reports_invalid_patch_diagnostics_without_raw_text() -> None:
    patch = sample_patch()
    invalid_patch = replace(patch, claims=[replace(patch.claims[0], confidence=2.0)])
    module = BaselineDspyExtractionModule(lambda boundary_input: invalid_patch)

    output = module.forward(DspyExtractionInput(paper_id=patch.paper_id))

    assert output.patch is invalid_patch
    assert output.schema_valid is False
    assert output.schema_diagnostic_count == 1
    assert "confidence 2.0 is outside [0.0, 1.0]" in output.schema_diagnostics[0]
    assert output.boundary_valid is False
    assert "Local markdown is enough" not in repr(output)


def test_boundary_reports_missing_expected_evidence_as_groundedness_failure() -> None:
    patch = sample_patch()
    module = BaselineDspyExtractionModule(lambda boundary_input: patch)

    output = module.forward(
        DspyExtractionInput(
            paper_id=patch.paper_id,
            expected_evidence_path_ids=frozenset({"evidence:missing:expected"}),
        )
    )

    assert output.schema_valid is True
    assert output.groundedness_valid is False
    assert output.groundedness_diagnostics["status"] == "invalid"
    assert output.groundedness_diagnostics["missing_expected_evidence_path_ids"] == [
        "evidence:missing:expected"
    ]
    assert output.boundary_valid is False


def test_boundary_handles_malformed_callable_output_and_does_not_import_dspy() -> None:
    import sys

    module = BaselineDspyExtractionModule(
        lambda boundary_input: cast(ExtractionPatch, {"not": "a patch"})
    )

    output = module(DspyExtractionInput(paper_id="2605.12345"))

    assert output.patch is None
    assert output.schema_valid is False
    assert output.groundedness_valid is False
    assert output.boundary_valid is False
    assert output.boundary_diagnostics == ["invalid_extractor_output:dict"]
    assert "dspy" not in sys.modules


def test_signature_spec_is_plain_local_metadata() -> None:
    spec = dspy_extraction_signature_spec()

    assert spec.optimizer_enabled is False
    assert spec.optimizer_policy == "disabled_fail_closed"
    assert spec.patch_schema == "ExtractionPatch"
    assert "paper_id" in spec.input_fields
    assert "groundedness_diagnostics" in spec.output_fields
