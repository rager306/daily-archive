# Formerly: src/arxiv_archive/dspy_extraction.py

"""Deterministic DSPy-compatible extraction boundary.

This module provides a typed, DSPy-like boundary around existing baseline
extractors that already return :class:`ExtractionPatch`. It intentionally does
not import DSPy, run optimizers, call language models, or write persistence.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from typing import Any

from research_graph.infrastructure.evaluation.evaluation_metrics import (
    GroundednessProxyResult,
    evaluate_groundedness_proxy,
    evaluate_schema_validity,
)
from research_graph.infrastructure.evaluation.scientific_extraction import ExtractionPatch

BOUNDARY_VERSION = "dspy_extraction_boundary.v1"


@dataclass(frozen=True)
class DspyExtractionInput:
    """Input for the deterministic extraction boundary.

    The payload is deliberately opaque to this boundary so callers can pass
    local fixture/configuration data to a baseline extractor without introducing
    DSPy runtime dependencies or alternate extraction schemas.
    """

    paper_id: str
    expected_evidence_path_ids: frozenset[str] = field(default_factory=frozenset)
    baseline_context: Mapping[str, Any] = field(default_factory=dict)
    optimizer_config: Mapping[str, Any] | None = None


@dataclass(frozen=True)
class DspyExtractionOutput:
    """Inspectable output from the deterministic extraction boundary.

    Diagnostics are limited to IDs, counts, booleans, status strings, and local
    exception type names. They must not contain paper body text, extracted claim
    text, secrets, prompts, or raw optimizer payloads.
    """

    patch: ExtractionPatch | None = field(repr=False)
    boundary_version: str
    optimizer_enabled: bool
    optimizer_name: str | None
    optimizer_rejection_reason: str | None
    schema_valid: bool
    schema_diagnostics: list[str]
    schema_diagnostic_count: int
    groundedness_valid: bool
    groundedness_diagnostics: dict[str, Any]
    boundary_valid: bool
    boundary_diagnostics: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class DspyExtractionBoundarySpec:
    """Plain local metadata describing the boundary surface."""

    boundary_version: str
    input_fields: tuple[str, ...]
    output_fields: tuple[str, ...]
    optimizer_enabled: bool
    optimizer_policy: str
    patch_schema: str


BaselineExtractionCallable = Callable[[DspyExtractionInput], ExtractionPatch]


class BaselineDspyExtractionModule:
    """DSPy-like module wrapper for deterministic baseline extraction.

    The public ``forward`` method mirrors DSPy module shape while keeping all
    optimizer and runtime behavior disabled. The wrapped callable must return
    the existing S04 ``ExtractionPatch`` schema.
    """

    def __init__(
        self,
        extractor: BaselineExtractionCallable,
        *,
        boundary_version: str = BOUNDARY_VERSION,
        optimizer_config: Mapping[str, Any] | None = None,
        optimizer_name: str | None = None,
    ) -> None:
        if optimizer_config is not None or optimizer_name is not None:
            raise ValueError(_optimizer_rejection_reason(optimizer_name))
        self._extractor = extractor
        self.boundary_version = boundary_version
        self.optimizer_enabled = False
        self.optimizer_name: str | None = None

    def forward(
        self,
        boundary_input: DspyExtractionInput,
        *,
        optimizer_config: Mapping[str, Any] | None = None,
        optimizer_name: str | None = None,
    ) -> DspyExtractionOutput:
        """Run a baseline extractor and evaluate schema/groundedness gates."""
        if (
            boundary_input.optimizer_config is not None
            or optimizer_config is not None
            or optimizer_name is not None
        ):
            raise ValueError(_optimizer_rejection_reason(optimizer_name))

        try:
            patch = self._extractor(boundary_input)
        except Exception as exc:  # noqa: BLE001 - boundary reports safe local failure shape
            return self._error_output(
                boundary_diagnostics=[f"extractor_error:{type(exc).__name__}"],
            )

        if not isinstance(patch, ExtractionPatch):
            return self._error_output(
                boundary_diagnostics=[f"invalid_extractor_output:{type(patch).__name__}"],
            )

        schema_result = evaluate_schema_validity(patch)
        groundedness_result = evaluate_groundedness_proxy(
            patch,
            expected_evidence_path_ids=boundary_input.expected_evidence_path_ids,
        )
        groundedness_diagnostics = _groundedness_diagnostics(groundedness_result)
        groundedness_valid = _groundedness_valid(groundedness_result)
        boundary_diagnostics = _boundary_diagnostics(boundary_input, patch)

        return DspyExtractionOutput(
            patch=patch,
            boundary_version=self.boundary_version,
            optimizer_enabled=False,
            optimizer_name=None,
            optimizer_rejection_reason=None,
            schema_valid=schema_result.valid,
            schema_diagnostics=list(schema_result.diagnostics),
            schema_diagnostic_count=schema_result.diagnostic_count,
            groundedness_valid=groundedness_valid,
            groundedness_diagnostics=groundedness_diagnostics,
            boundary_valid=schema_result.valid and groundedness_valid and not boundary_diagnostics,
            boundary_diagnostics=boundary_diagnostics,
        )

    def __call__(
        self,
        boundary_input: DspyExtractionInput,
        *,
        optimizer_config: Mapping[str, Any] | None = None,
        optimizer_name: str | None = None,
    ) -> DspyExtractionOutput:
        """Delegate calls to ``forward`` for DSPy-like ergonomics."""
        return self.forward(
            boundary_input,
            optimizer_config=optimizer_config,
            optimizer_name=optimizer_name,
        )

    def signature_spec(self) -> DspyExtractionBoundarySpec:
        """Return local metadata without importing DSPy."""
        return dspy_extraction_signature_spec(boundary_version=self.boundary_version)

    def _error_output(self, *, boundary_diagnostics: list[str]) -> DspyExtractionOutput:
        return DspyExtractionOutput(
            patch=None,
            boundary_version=self.boundary_version,
            optimizer_enabled=False,
            optimizer_name=None,
            optimizer_rejection_reason=None,
            schema_valid=False,
            schema_diagnostics=[],
            schema_diagnostic_count=0,
            groundedness_valid=False,
            groundedness_diagnostics=_empty_groundedness_diagnostics(),
            boundary_valid=False,
            boundary_diagnostics=boundary_diagnostics,
        )


def dspy_extraction_signature_spec(
    *, boundary_version: str = BOUNDARY_VERSION
) -> DspyExtractionBoundarySpec:
    """Return a plain, dependency-free boundary signature description."""
    return DspyExtractionBoundarySpec(
        boundary_version=boundary_version,
        input_fields=(
            "paper_id",
            "expected_evidence_path_ids",
            "baseline_context",
            "optimizer_config",
        ),
        output_fields=(
            "patch",
            "boundary_version",
            "optimizer_enabled",
            "optimizer_name",
            "optimizer_rejection_reason",
            "schema_valid",
            "schema_diagnostics",
            "schema_diagnostic_count",
            "groundedness_valid",
            "groundedness_diagnostics",
            "boundary_valid",
            "boundary_diagnostics",
        ),
        optimizer_enabled=False,
        optimizer_policy="disabled_fail_closed",
        patch_schema="ExtractionPatch",
    )


def _optimizer_rejection_reason(optimizer_name: str | None) -> str:
    requested = optimizer_name or "optimizer_config"
    return f"DSPy optimizer runtime is disabled for {BOUNDARY_VERSION}; rejected {requested}"


def _groundedness_valid(result: GroundednessProxyResult) -> bool:
    return (
        not result.missing_expected_evidence_path_ids
        and not result.unexpected_evidence_path_ids
        and not result.missing_evidence_path_draft_ids
    )


def _groundedness_diagnostics(result: GroundednessProxyResult) -> dict[str, Any]:
    return {
        "claim_count": result.claim_count,
        "entity_count": result.entity_count,
        "relation_count": result.relation_count,
        "evidence_backed_claim_count": result.evidence_backed_claim_count,
        "evidence_backed_entity_count": result.evidence_backed_entity_count,
        "evidence_backed_relation_count": result.evidence_backed_relation_count,
        "derived_evidence_path_ids": list(result.derived_evidence_path_ids),
        "missing_expected_evidence_path_ids": list(result.missing_expected_evidence_path_ids),
        "unexpected_evidence_path_ids": list(result.unexpected_evidence_path_ids),
        "missing_evidence_path_draft_ids": list(result.missing_evidence_path_draft_ids),
        "status": "valid" if _groundedness_valid(result) else "invalid",
    }


def _empty_groundedness_diagnostics() -> dict[str, Any]:
    return {
        "claim_count": 0,
        "entity_count": 0,
        "relation_count": 0,
        "evidence_backed_claim_count": 0,
        "evidence_backed_entity_count": 0,
        "evidence_backed_relation_count": 0,
        "derived_evidence_path_ids": [],
        "missing_expected_evidence_path_ids": [],
        "unexpected_evidence_path_ids": [],
        "missing_evidence_path_draft_ids": [],
        "status": "invalid",
    }


def _boundary_diagnostics(boundary_input: DspyExtractionInput, patch: ExtractionPatch) -> list[str]:
    diagnostics: list[str] = []
    if patch.source_id != boundary_input.paper_id:
        diagnostics.append("patch_paper_id_mismatch")
    return diagnostics


__all__ = [
    "BOUNDARY_VERSION",
    "BaselineDspyExtractionModule",
    "BaselineExtractionCallable",
    "DspyExtractionBoundarySpec",
    "DspyExtractionInput",
    "DspyExtractionOutput",
    "dspy_extraction_signature_spec",
]
