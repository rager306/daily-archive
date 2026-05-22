from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path

from arxiv_archive.chunk_repair_contract import (
    CHUNK_REPAIR_CONTRACT_VERSION,
    scan_forbidden_payload_keys,
    validate_chunk_repair_contract,
    validation_to_dict,
)

FIXTURE = Path("tests/fixtures/chunk_repair_contract.json")


def _fixture() -> dict[str, object]:
    return json.loads(FIXTURE.read_text(encoding="utf-8"))


def _reasons(payload: dict[str, object]) -> set[str]:
    return set(validate_chunk_repair_contract(payload).refusal_counts)


def test_synthetic_fixture_passes_and_preserves_false_safety_flags() -> None:
    result = validate_chunk_repair_contract(_fixture())
    serialized = validation_to_dict(result)

    assert result.passed is True
    assert result.valid_contract is True
    assert result.target_count == 2
    assert result.import_eligible_count == 0
    assert result.production_write_count == 0
    assert result.semantic_ready_count == 0
    assert result.diagnostics == []
    assert serialized["schema_version"] == "chunk-repair-contract-validation.v1"
    assert serialized["contract_version"] == CHUNK_REPAIR_CONTRACT_VERSION
    assert serialized["raw_text_included"] is False
    assert serialized["chunk_text_included"] is False
    assert serialized["embeddings_included"] is False
    assert serialized["vectors_included"] is False
    assert serialized["ladybugdb_written"] is False
    assert serialized["production_import_attempted"] is False


def test_empty_repair_targets_are_valid_for_contract_artifact() -> None:
    payload = _fixture()
    payload["repair_targets"] = []
    payload["diagnostics"] = {
        **payload["diagnostics"],
        "target_count": 0,
        "pending_review_count": 0,
    }

    result = validate_chunk_repair_contract(payload)

    assert result.passed is True
    assert result.target_count == 0


def test_missing_header_and_target_ids_are_rejected_without_raising() -> None:
    payload = _fixture()
    payload.pop("schema_version")
    target = deepcopy(payload["repair_targets"])[0]
    target.pop("target_id")
    target.pop("locator_id")
    payload["repair_targets"] = [target]

    reasons = _reasons(payload)

    assert "schema_version_mismatch" in reasons
    assert "missing_target_id" in reasons
    assert "missing_locator_id" in reasons


def test_expected_audit_rejects_unresolved_stable_references() -> None:
    payload = _fixture()

    result = validate_chunk_repair_contract(
        payload,
        expected_audit={
            "locator_ids": ["different-locator"],
            "source_ids": ["different-source"],
            "paper_ids": ["synthetic-paper-1"],
        },
    )

    assert "unresolved_locator_id" in result.refusal_counts
    assert "unresolved_source_id" in result.refusal_counts


def test_forbidden_payload_key_injection_reports_path_not_value() -> None:
    payload = _fixture()
    payload["repair_targets"][0]["review_packet"] = {"nested": {"chunk_text": "NEVER LEAK THIS RAW VALUE"}}

    result = validate_chunk_repair_contract(payload)
    serialized = validation_to_dict(result)
    rendered = json.dumps(serialized)

    assert result.passed is False
    assert "raw_text_leakage" in result.refusal_counts
    assert "/repair_targets/0/review_packet/nested/chunk_text" in rendered
    assert "NEVER LEAK THIS RAW VALUE" not in rendered


def test_forbidden_key_scanner_is_redacted_and_recursive() -> None:
    findings = scan_forbidden_payload_keys({"outer": [{"api_key": "SECRET"}, {"vectors": [0.1]}]})
    rendered = json.dumps([finding.__dict__ for finding in findings])

    assert [finding.path for finding in findings] == ["/outer/0/api_key", "/outer/1/vectors"]
    assert "SECRET" not in rendered
    assert "0.1" not in rendered


def test_safety_flags_cannot_request_import_fact_write_or_semantic_readiness() -> None:
    payload = _fixture()
    target = deepcopy(payload["repair_targets"])[0]
    target["safety_boundaries"]["import_eligible"] = True
    target["safety_boundaries"]["promoted_to_fact"] = True
    target["safety_boundaries"]["trusted_kg_import_allowed"] = True
    target["safety_boundaries"]["production_write_attempted"] = True
    target["safety_boundaries"]["ladybugdb_written"] = True
    target["safety_boundaries"]["semantic_ready_for_kg"] = True
    payload["repair_targets"] = [target]

    reasons = _reasons(payload)

    assert "import_eligible_true" in reasons
    assert "promoted_to_fact_true" in reasons
    assert "trusted_kg_import_allowed_true" in reasons
    assert "production_write_attempted" in reasons
    assert "ladybugdb_written" in reasons
    assert "semantic_ready_for_kg_true" in reasons


def test_route_state_vocabulary_and_confusion_are_rejected() -> None:
    payload = _fixture()
    target = deepcopy(payload["repair_targets"])[0]
    target["route"] = "claim_extraction"
    target["state"] = "ok_for_graph"
    payload["repair_targets"] = [target]

    reasons = _reasons(payload)

    assert "invalid_repair_route" in reasons
    assert "invalid_repair_state" in reasons

    payload = _fixture()
    target = deepcopy(payload["repair_targets"])[0]
    target["route"] = "retrieval_context"
    target["state"] = "ambiguous_span"
    payload["repair_targets"] = [target]

    assert "route_state_confusion" in _reasons(payload)


def test_malformed_spans_are_rejected_by_code_and_path() -> None:
    payload = _fixture()
    span = deepcopy(payload["repair_targets"])[0]["source_spans"][0]
    span["coordinate_space"] = "pdf_pixel_box"
    span["char_start"] = 80
    span["char_end"] = 10
    span.pop("span_hash")
    target = deepcopy(payload["repair_targets"])[0]
    target["source_spans"] = [span]
    payload["repair_targets"] = [target]

    result = validate_chunk_repair_contract(payload)
    rendered = json.dumps(validation_to_dict(result))

    assert "unsupported_coordinate_space" in result.refusal_counts
    assert "invalid_coordinate_bounds" in result.refusal_counts
    assert "missing_span_hash" in result.refusal_counts
    assert "/repair_targets/0/source_spans/0/span_hash" in rendered


def test_accepted_review_status_requires_reviewer_fields() -> None:
    payload = _fixture()
    target = deepcopy(payload["repair_targets"])[0]
    target["review_status"] = "accepted"
    target["reviewer"] = {"reviewer_id": "reviewer-1"}
    payload["repair_targets"] = [target]

    reasons = _reasons(payload)

    assert "missing_reviewed_at" in reasons
    assert "missing_decision_summary" in reasons
    assert "missing_evidence_checked" in reasons


def test_diagnostics_count_drift_is_rejected() -> None:
    payload = _fixture()
    payload["diagnostics"]["target_count"] = 99
    payload["diagnostics"]["import_eligible_count"] = 1

    reasons = _reasons(payload)

    assert "diagnostics_target_count_mismatch" in reasons
    assert "diagnostics_import_count_mismatch" in reasons
