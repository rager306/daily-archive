"""Validation batch state contract for iterative corpus validation.

This module is deliberately pure and local: it models resumable validation batch
state, redaction/write-safety flags, and contradiction diagnostics. It does not
perform source acquisition, paper conversion, scan execution, KG import, or
LadybugDB writes.
"""

from __future__ import annotations

import json
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m007-validation-batch-state.v1"
CONTRACT_SCHEMA_VERSION = "m007-validation-batch-contract.v1"

PHASES = frozenset(
    {
        "planned",
        "initialized",
        "source_preflighted",
        "source_ready",
        "source_blocked",
        "scan_ready",
        "scanned",
        "review_required",
        "reviewed",
        "complete",
    }
)

SELECTION_ROLES = frozenset(
    {
        "baseline_overlap",
        "deterministic_expansion",
        "retry",
        "repaired",
        "excluded",
        "manual_review_target",
    }
)

SAFETY_FLAG_KEYS = (
    "raw_text_included",
    "chunk_text_included",
    "raw_binary_included",
    "base64_included",
    "embeddings_included",
    "vectors_included",
    "secrets_included",
    "optimizer_traces_included",
    "production_import_attempted",
    "ladybugdb_written",
)


@dataclass(frozen=True)
class ValidationSafetyFlags:
    raw_text_included: bool = False
    chunk_text_included: bool = False
    raw_binary_included: bool = False
    base64_included: bool = False
    embeddings_included: bool = False
    vectors_included: bool = False
    secrets_included: bool = False
    optimizer_traces_included: bool = False
    production_import_attempted: bool = False
    ladybugdb_written: bool = False


@dataclass(frozen=True)
class SelectedPaper:
    paper_id: str
    selection_role: str
    rank: int | None = None
    risk_tags: tuple[str, ...] = ()
    source_paths: dict[str, str] = field(default_factory=dict)
    notes: tuple[str, ...] = ()


@dataclass(frozen=True)
class SourceReadiness:
    markdown_present: bool = False
    markdown_quality_accepted: bool = False
    pdf_present: bool = False
    pdf_missing: bool = False
    conversion_repaired: bool = False
    conversion_failed: bool = False
    unavailable_source: bool = False
    ready_for_markdown_scan: bool = False
    loader_provenance_by_role: dict[str, dict[str, Any]] = field(default_factory=dict)


@dataclass(frozen=True)
class ScanArtifactPaths:
    aggregate_summary_json: str | None = None
    per_paper_diagnostics_jsonl: str | None = None
    delta_report_json: str | None = None
    outlier_report_json: str | None = None
    review_summary_md: str | None = None


@dataclass(frozen=True)
class ReviewState:
    verdict: str | None = None
    review_required: bool = False
    blockers: tuple[str, ...] = ()
    warnings: tuple[str, ...] = ()


@dataclass(frozen=True)
class BatchRecommendation:
    next_action: str = "review_required"
    reason: str = "Batch has not completed review."
    recommended_next_batch_size: int | None = None


@dataclass(frozen=True)
class ValidationBatchState:
    batch_id: str
    phase: str = "planned"
    selected_papers: tuple[SelectedPaper, ...] = ()
    input_manifests: tuple[str, ...] = ()
    artifact_paths: ScanArtifactPaths = field(default_factory=ScanArtifactPaths)
    source_readiness_by_paper: dict[str, SourceReadiness] = field(default_factory=dict)
    review: ReviewState = field(default_factory=ReviewState)
    recommendation: BatchRecommendation = field(default_factory=BatchRecommendation)
    safety: ValidationSafetyFlags = field(default_factory=ValidationSafetyFlags)
    diagnostics: tuple[dict[str, str], ...] = ()
    schema_version: str = SCHEMA_VERSION


def default_safety_flags() -> dict[str, bool]:
    """Return the standard redaction/write-safety flags, all set to false."""
    return asdict(ValidationSafetyFlags())


def validation_safety_flags_from_dict(payload: dict[str, Any]) -> ValidationSafetyFlags:
    return ValidationSafetyFlags(**{key: bool(payload.get(key, False)) for key in SAFETY_FLAG_KEYS})


def selected_paper_from_dict(payload: dict[str, Any]) -> SelectedPaper:
    return SelectedPaper(
        paper_id=str(payload["paper_id"]),
        selection_role=str(payload["selection_role"]),
        rank=payload.get("rank"),
        risk_tags=tuple(str(value) for value in payload.get("risk_tags", ())),
        source_paths={str(key): str(value) for key, value in payload.get("source_paths", {}).items()},
        notes=tuple(str(value) for value in payload.get("notes", ())),
    )


def source_readiness_from_dict(payload: dict[str, Any]) -> SourceReadiness:
    return SourceReadiness(
        markdown_present=bool(payload.get("markdown_present", False)),
        markdown_quality_accepted=bool(payload.get("markdown_quality_accepted", False)),
        pdf_present=bool(payload.get("pdf_present", False)),
        pdf_missing=bool(payload.get("pdf_missing", False)),
        conversion_repaired=bool(payload.get("conversion_repaired", False)),
        conversion_failed=bool(payload.get("conversion_failed", False)),
        unavailable_source=bool(payload.get("unavailable_source", False)),
        ready_for_markdown_scan=bool(payload.get("ready_for_markdown_scan", False)),
        loader_provenance_by_role={
            str(role): dict(provenance)
            for role, provenance in payload.get("loader_provenance_by_role", {}).items()
            if isinstance(provenance, dict)
        },
    )


def scan_artifact_paths_from_dict(payload: dict[str, Any]) -> ScanArtifactPaths:
    return ScanArtifactPaths(
        aggregate_summary_json=payload.get("aggregate_summary_json"),
        per_paper_diagnostics_jsonl=payload.get("per_paper_diagnostics_jsonl"),
        delta_report_json=payload.get("delta_report_json"),
        outlier_report_json=payload.get("outlier_report_json"),
        review_summary_md=payload.get("review_summary_md"),
    )


def review_state_from_dict(payload: dict[str, Any]) -> ReviewState:
    return ReviewState(
        verdict=payload.get("verdict"),
        review_required=bool(payload.get("review_required", False)),
        blockers=tuple(str(value) for value in payload.get("blockers", ())),
        warnings=tuple(str(value) for value in payload.get("warnings", ())),
    )


def batch_recommendation_from_dict(payload: dict[str, Any]) -> BatchRecommendation:
    return BatchRecommendation(
        next_action=str(payload.get("next_action", "review_required")),
        reason=str(payload.get("reason", "Batch has not completed review.")),
        recommended_next_batch_size=payload.get("recommended_next_batch_size"),
    )


def batch_state_to_dict(state: ValidationBatchState) -> dict[str, Any]:
    """Serialize batch state into deterministic JSON-native data."""
    return {
        "schema_version": state.schema_version,
        "batch_id": state.batch_id,
        "phase": state.phase,
        "selected_papers": [asdict(paper) for paper in state.selected_papers],
        "input_manifests": list(state.input_manifests),
        "artifact_paths": asdict(state.artifact_paths),
        "source_readiness_by_paper": {
            paper_id: asdict(readiness) for paper_id, readiness in sorted(state.source_readiness_by_paper.items())
        },
        "review": asdict(state.review),
        "recommendation": asdict(state.recommendation),
        "safety": asdict(state.safety),
        "diagnostics": [dict(diagnostic) for diagnostic in state.diagnostics],
    }


def batch_state_from_dict(payload: dict[str, Any]) -> ValidationBatchState:
    """Deserialize a validation batch state from JSON-native data."""
    schema_version = str(payload.get("schema_version", ""))
    if schema_version != SCHEMA_VERSION:
        raise ValueError(f"unsupported validation batch state schema: {schema_version}")
    return ValidationBatchState(
        schema_version=schema_version,
        batch_id=str(payload["batch_id"]),
        phase=str(payload.get("phase", "planned")),
        selected_papers=tuple(selected_paper_from_dict(value) for value in payload.get("selected_papers", ())),
        input_manifests=tuple(str(value) for value in payload.get("input_manifests", ())),
        artifact_paths=scan_artifact_paths_from_dict(payload.get("artifact_paths", {})),
        source_readiness_by_paper={
            str(paper_id): source_readiness_from_dict(readiness)
            for paper_id, readiness in payload.get("source_readiness_by_paper", {}).items()
        },
        review=review_state_from_dict(payload.get("review", {})),
        recommendation=batch_recommendation_from_dict(payload.get("recommendation", {})),
        safety=validation_safety_flags_from_dict(payload.get("safety", {})),
        diagnostics=tuple(dict(value) for value in payload.get("diagnostics", ())),
    )


def write_batch_state(state: ValidationBatchState, path: str | Path) -> Path:
    output_path = Path(path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(batch_state_to_dict(state), indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return output_path


def read_batch_state(path: str | Path) -> ValidationBatchState:
    payload = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"expected JSON object at {path}")
    return batch_state_from_dict(payload)


def validate_safety_flags(flags: ValidationSafetyFlags | dict[str, bool]) -> list[dict[str, str]]:
    values = asdict(flags) if isinstance(flags, ValidationSafetyFlags) else flags
    diagnostics: list[dict[str, str]] = []
    for key in SAFETY_FLAG_KEYS:
        if bool(values.get(key, False)):
            diagnostics.append(
                _diagnostic(
                    severity="blocker",
                    code=f"unsafe_{key}",
                    message=f"Safety flag {key} must remain false for validation batches.",
                    recommended_action="Stop the batch and inspect artifact generation before continuing.",
                )
            )
    return diagnostics


def detect_source_contradictions(paper: SelectedPaper, readiness: SourceReadiness) -> list[dict[str, str]]:
    diagnostics: list[dict[str, str]] = []
    risk_tags = set(paper.risk_tags)
    if readiness.ready_for_markdown_scan and not readiness.markdown_present:
        diagnostics.append(
            _diagnostic(
                severity="blocker",
                code="ready_without_markdown",
                paper_id=paper.paper_id,
                message="Paper is marked ready for Markdown scan but Markdown is not present.",
                recommended_action="Run source preflight/acquisition before scanning.",
            )
        )
    if readiness.ready_for_markdown_scan and not readiness.markdown_quality_accepted:
        diagnostics.append(
            _diagnostic(
                severity="blocker",
                code="ready_without_markdown_quality",
                paper_id=paper.paper_id,
                message="Paper is marked ready for Markdown scan but Markdown quality is not accepted.",
                recommended_action="Repair or reject the Markdown before scanning.",
            )
        )
    if readiness.ready_for_markdown_scan and "missing_markdown" in risk_tags:
        diagnostics.append(
            _diagnostic(
                severity="warning",
                code="ready_with_missing_markdown_risk_tag",
                paper_id=paper.paper_id,
                message="Paper is scan-ready but still carries a missing_markdown risk tag.",
                recommended_action="Resolve whether the risk tag is historical or still actionable.",
            )
        )
    if readiness.pdf_present and readiness.pdf_missing:
        diagnostics.append(
            _diagnostic(
                severity="warning",
                code="conflicting_pdf_state",
                paper_id=paper.paper_id,
                message="Paper is marked with both pdf_present and pdf_missing.",
                recommended_action="Refresh PDF preflight state before review.",
            )
        )
    if readiness.conversion_repaired and readiness.conversion_failed:
        diagnostics.append(
            _diagnostic(
                severity="blocker",
                code="conflicting_conversion_state",
                paper_id=paper.paper_id,
                message="Paper is marked with both conversion_repaired and conversion_failed.",
                recommended_action="Resolve conversion provenance before scanning.",
            )
        )
    if readiness.unavailable_source and readiness.ready_for_markdown_scan:
        diagnostics.append(
            _diagnostic(
                severity="blocker",
                code="ready_with_unavailable_source",
                paper_id=paper.paper_id,
                message="Paper is marked source-unavailable and ready for Markdown scan.",
                recommended_action="Correct source readiness state before scanning.",
            )
        )
    return diagnostics


def build_batch_diagnostics(state: ValidationBatchState) -> list[dict[str, str]]:
    diagnostics: list[dict[str, str]] = []
    if state.phase not in PHASES:
        diagnostics.append(
            _diagnostic(
                severity="blocker",
                code="unknown_phase",
                message=f"Unknown validation batch phase: {state.phase}",
                recommended_action="Use a phase from the validation batch contract.",
            )
        )
    for paper in state.selected_papers:
        if paper.selection_role not in SELECTION_ROLES:
            diagnostics.append(
                _diagnostic(
                    severity="warning",
                    code="unknown_selection_role",
                    paper_id=paper.paper_id,
                    message=f"Unknown selection role: {paper.selection_role}",
                    recommended_action="Normalize the selection role before batch review.",
                )
            )
        readiness = state.source_readiness_by_paper.get(paper.paper_id)
        if readiness is not None:
            diagnostics.extend(detect_source_contradictions(paper, readiness))
    diagnostics.extend(validate_safety_flags(state.safety))
    return diagnostics


def build_contract_response(command: str, *, status: str = "contract_only") -> dict[str, Any]:
    """Return a redacted response for validation-batch contract and stubs."""
    return {
        "schema_version": CONTRACT_SCHEMA_VERSION,
        "command": command,
        "status": status,
        "boundary": "No production KG import; validation-batch commands are operational diagnostics only.",
        "real_source_acquisition_performed": False,
        "real_scan_performed": False,
        "real_review_mutation_performed": False,
        **default_safety_flags(),
    }


def _diagnostic(
    *,
    severity: str,
    code: str,
    message: str,
    recommended_action: str,
    paper_id: str | None = None,
) -> dict[str, str]:
    diagnostic = {
        "severity": severity,
        "code": code,
        "message": message,
        "recommended_action": recommended_action,
    }
    if paper_id is not None:
        diagnostic["paper_id"] = paper_id
    return diagnostic
