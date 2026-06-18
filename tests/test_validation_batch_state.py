from __future__ import annotations

import json
from dataclasses import replace
from pathlib import Path

from research_graph.workflows.validation.batch_state import (
    BatchRecommendation,
    ScanArtifactPaths,
    SelectedPaper,
    SourceReadiness,
    ValidationBatchState,
    ValidationSafetyFlags,
    batch_state_from_dict,
    batch_state_to_dict,
    build_batch_diagnostics,
    build_contract_response,
    default_safety_flags,
    detect_source_contradictions,
    read_batch_state,
    validate_safety_flags,
    write_batch_state,
)


def test_default_safety_flags_are_false() -> None:
    flags = default_safety_flags()

    assert flags
    assert all(value is False for value in flags.values())
    assert "production_import_attempted" in flags
    assert "ladybugdb_written" in flags


def test_validation_batch_state_round_trips_json(tmp_path: Path) -> None:
    state = ValidationBatchState(
        batch_id="b001",
        phase="source_ready",
        selected_papers=(
            SelectedPaper(
                paper_id="2605.00001v1",
                rank=1,
                selection_role="deterministic_expansion",
                risk_tags=("missing_pdf",),
                source_paths={"workspace": "/tmp/paper"},
            ),
        ),
        input_manifests=("manifest.json",),
        artifact_paths=ScanArtifactPaths(aggregate_summary_json="summary.json"),
        source_readiness_by_paper={
            "2605.00001v1": SourceReadiness(
                markdown_present=True,
                markdown_quality_accepted=True,
                pdf_missing=True,
                ready_for_markdown_scan=True,
                loader_provenance_by_role={"markdown": {"source_type": "markdown", "sha256": "a" * 64}},
            )
        },
        recommendation=BatchRecommendation(
            next_action="run_scan",
            reason="Source preflight passed.",
            recommended_next_batch_size=10,
        ),
    )

    payload = batch_state_to_dict(state)
    restored = batch_state_from_dict(json.loads(json.dumps(payload)))
    path = write_batch_state(restored, tmp_path / "batch-state.json")

    assert restored == state
    assert read_batch_state(path) == state
    assert payload["schema_version"] == "m007-validation-batch-state.v1"
    assert payload["safety"]["raw_text_included"] is False
    assert payload["source_readiness_by_paper"]["2605.00001v1"]["loader_provenance_by_role"]["markdown"]["sha256"] == "a" * 64


def test_detect_source_contradictions_for_ready_missing_markdown() -> None:
    paper = SelectedPaper(
        paper_id="2605.00002v1",
        selection_role="deterministic_expansion",
        risk_tags=("missing_markdown",),
    )
    readiness = SourceReadiness(ready_for_markdown_scan=True)

    diagnostics = detect_source_contradictions(paper, readiness)

    codes = {diagnostic["code"] for diagnostic in diagnostics}
    assert "ready_without_markdown" in codes
    assert "ready_without_markdown_quality" in codes
    assert "ready_with_missing_markdown_risk_tag" in codes
    assert all(diagnostic["paper_id"] == "2605.00002v1" for diagnostic in diagnostics)


def test_detect_source_contradictions_for_conflicting_states() -> None:
    paper = SelectedPaper(paper_id="2605.00003v1", selection_role="retry")
    readiness = SourceReadiness(
        markdown_present=True,
        markdown_quality_accepted=True,
        pdf_present=True,
        pdf_missing=True,
        conversion_repaired=True,
        conversion_failed=True,
        unavailable_source=True,
        ready_for_markdown_scan=True,
    )

    diagnostics = detect_source_contradictions(paper, readiness)

    codes = {diagnostic["code"] for diagnostic in diagnostics}
    assert "conflicting_pdf_state" in codes
    assert "conflicting_conversion_state" in codes
    assert "ready_with_unavailable_source" in codes


def test_build_batch_diagnostics_includes_safety_and_phase_issues() -> None:
    state = ValidationBatchState(
        batch_id="b002",
        phase="unknown",
        selected_papers=(SelectedPaper(paper_id="2605.00004v1", selection_role="mystery"),),
        safety=replace(ValidationSafetyFlags(), chunk_text_included=True),
    )

    diagnostics = build_batch_diagnostics(state)

    codes = {diagnostic["code"] for diagnostic in diagnostics}
    assert "unknown_phase" in codes
    assert "unknown_selection_role" in codes
    assert "unsafe_chunk_text_included" in codes


def test_clean_batch_state_has_no_diagnostics() -> None:
    state = ValidationBatchState(
        batch_id="b003",
        phase="source_ready",
        selected_papers=(SelectedPaper(paper_id="2605.00005v1", selection_role="baseline_overlap"),),
        source_readiness_by_paper={
            "2605.00005v1": SourceReadiness(
                markdown_present=True,
                markdown_quality_accepted=True,
                pdf_missing=True,
                ready_for_markdown_scan=True,
            )
        },
    )

    assert build_batch_diagnostics(state) == []


def test_contract_response_is_safe_and_contract_only() -> None:
    response = build_contract_response("validation-batch contract")

    assert response["status"] == "contract_only"
    assert response["real_source_acquisition_performed"] is False
    assert response["real_scan_performed"] is False
    assert response["production_import_attempted"] is False
    assert "No production KG import" in response["boundary"]


def test_serialized_state_does_not_include_raw_or_chunk_text_fixture() -> None:
    forbidden_raw_text = "This raw paper paragraph must never be serialized"
    forbidden_chunk_text = "This chunk text must never be serialized"
    state = ValidationBatchState(
        batch_id="b004",
        selected_papers=(
            SelectedPaper(
                paper_id="2605.00006v1",
                selection_role="manual_review_target",
                notes=("review only",),
            ),
        ),
    )

    serialized = json.dumps(batch_state_to_dict(state), sort_keys=True)

    assert forbidden_raw_text not in serialized
    assert forbidden_chunk_text not in serialized


def test_validate_safety_flags_accepts_dict() -> None:
    diagnostics = validate_safety_flags({"vectors_included": True})

    assert diagnostics == [
        {
            "severity": "blocker",
            "code": "unsafe_vectors_included",
            "message": "Safety flag vectors_included must remain false for validation batches.",
            "recommended_action": "Stop the batch and inspect artifact generation before continuing.",
        }
    ]
