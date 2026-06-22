from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path

from research_graph.infrastructure.repair.chunk_repair_contract import (
    CHUNK_REPAIR_CONTRACT_VERSION,
    build_chunk_repair_contract_from_audit,
    expected_audit_from_contract,
    render_chunk_repair_contract_markdown,
    scan_forbidden_payload_keys,
    validate_chunk_repair_contract,
    validate_chunk_repair_contract_markdown,
    validate_locator_evidence_audit_for_repair_contract,
    validation_to_dict,
)

# pyrefly: ignore [missing-import]
from scripts.render_chunk_repair_contract import main as render_contract_main

FIXTURE = Path("tests/fixtures/chunk_repair_contract.json")


def _fixture() -> dict[str, object]:
    return json.loads(FIXTURE.read_text(encoding="utf-8"))


def _audit_fixture() -> dict[str, object]:
    return {
        "schema_version": "locator_evidence_audit.v1",
        "input_schema_version": "candidate_locator_protocol.v1",
        "strict": True,
        "input_path": "tests/fixtures/synthetic-locator-batch.json",
        "first_proof_invariants": {
            "schema_version": "candidate_locator_protocol.v1",
            "paper_count": 1,
            "source_count": 1,
            "locator_count": 2,
            "located_count": 2,
            "review_required_count": 0,
            "missing_span_count": 0,
            "ambiguous_span_count": 1,
            "conflicting_evidence_count": 0,
            "retrieval_only_count": 1,
            "repair_required_count": 0,
            "import_eligible_count": 0,
            "promoted_to_fact_count": 0,
        },
        "stable_ids": {
            "source_ids": ["source-synthetic-paper-1-full-text"],
            "locator_ids": [
                "m021-synthetic-paper-1-claim-001",
                "m021-synthetic-paper-1-retrieval-001",
            ],
            "span_ids": [
                "m021-synthetic-paper-1-claim-001-span-001",
                "m021-synthetic-paper-1-retrieval-001-span-001",
            ],
        },
        "distributions": {},
        "diagnostic_code_classes": {},
        "source_span_coverage": {},
        "source_ledger_safety": {},
        "repair_context_gaps": {},
        "safety_blockers": {
            "validator_diagnostics": [],
            "forbidden_payload_key_paths": [],
            "unsafe_safety_flag_paths": [],
            "summary_drift": [],
            "invariant_drift": [],
            "no_import_blocker_intact": True,
        },
    }


def _reasons(payload: dict[str, object]) -> set[str]:
    return set(validate_chunk_repair_contract(payload).refusal_counts)


def _single_target_batch_contract() -> dict[str, object]:
    contract = build_chunk_repair_contract_from_audit(
        _audit_fixture(), source_audit_path="tests/fixtures/audit.json"
    )
    target = deepcopy(_fixture()["repair_targets"])[0]  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    contract["repair_targets"] = [target]
    contract["diagnostics"] = {
        **contract["diagnostics"],
        "target_count": 1,
        "pending_review_count": 1,
    }
    return contract


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
        # pyrefly: ignore [invalid-argument]
        **payload["diagnostics"],  # ty:ignore[invalid-argument-type]
        "target_count": 0,
        "pending_review_count": 0,
    }

    result = validate_chunk_repair_contract(payload)

    assert result.passed is True
    assert result.target_count == 0


def test_missing_header_and_target_ids_are_rejected_without_raising() -> None:
    payload = _fixture()
    payload.pop("schema_version")
    target = deepcopy(payload["repair_targets"])[0]  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
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


def test_batch_contract_accepts_target_paper_from_expected_audit() -> None:
    payload = _single_target_batch_contract()

    result = validate_chunk_repair_contract(
        payload, expected_audit=expected_audit_from_contract(payload)
    )

    assert result.passed is True
    assert "paper_id_mismatch" not in result.refusal_counts
    assert "unresolved_paper_id" not in result.refusal_counts


def test_batch_contract_rejects_unknown_target_paper_id() -> None:
    payload = _single_target_batch_contract()
    payload["repair_targets"][0]["paper_id"] = "unknown-paper"  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]

    result = validate_chunk_repair_contract(
        payload, expected_audit=expected_audit_from_contract(payload)
    )

    assert result.passed is False
    assert "unresolved_paper_id" in result.refusal_counts


def test_missing_expected_audit_still_enforces_package_paper_id() -> None:
    payload = _single_target_batch_contract()

    result = validate_chunk_repair_contract(payload)

    assert result.passed is False
    assert "paper_id_mismatch" in result.refusal_counts


def test_forbidden_payload_key_injection_reports_path_not_value() -> None:
    payload = _fixture()
    payload["repair_targets"][0]["review_packet"] = {  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
        "nested": {"chunk_text": "NEVER LEAK THIS RAW VALUE"}
    }

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
    target = deepcopy(payload["repair_targets"])[0]  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
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
    target = deepcopy(payload["repair_targets"])[0]  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    target["route"] = "claim_extraction"
    target["state"] = "ok_for_graph"
    payload["repair_targets"] = [target]

    reasons = _reasons(payload)

    assert "invalid_repair_route" in reasons
    assert "invalid_repair_state" in reasons

    payload = _fixture()
    target = deepcopy(payload["repair_targets"])[0]  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    target["route"] = "retrieval_context"
    target["state"] = "ambiguous_span"
    payload["repair_targets"] = [target]

    assert "route_state_confusion" in _reasons(payload)


def test_malformed_spans_are_rejected_by_code_and_path() -> None:
    payload = _fixture()
    span = deepcopy(payload["repair_targets"])[0]["source_spans"][0]  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    span["coordinate_space"] = "pdf_pixel_box"
    span["char_start"] = 80
    span["char_end"] = 10
    span.pop("span_hash")
    target = deepcopy(payload["repair_targets"])[0]  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
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
    target = deepcopy(payload["repair_targets"])[0]  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    target["review_status"] = "accepted"
    target["reviewer"] = {"reviewer_id": "reviewer-1"}
    payload["repair_targets"] = [target]

    reasons = _reasons(payload)

    assert "missing_reviewed_at" in reasons
    assert "missing_decision_summary" in reasons
    assert "missing_evidence_checked" in reasons


def test_diagnostics_count_drift_is_rejected() -> None:
    payload = _fixture()
    # pyrefly: ignore [unsupported-operation]
    payload["diagnostics"]["target_count"] = 99  # ty:ignore[invalid-assignment]
    # pyrefly: ignore [unsupported-operation]
    payload["diagnostics"]["import_eligible_count"] = 1  # ty:ignore[invalid-assignment]

    reasons = _reasons(payload)

    assert "diagnostics_target_count_mismatch" in reasons
    assert "diagnostics_import_count_mismatch" in reasons


def test_build_contract_from_audit_is_review_only_and_validator_clean() -> None:
    contract = build_chunk_repair_contract_from_audit(
        _audit_fixture(), source_audit_path="tests/fixtures/audit.json"
    )
    result = validate_chunk_repair_contract(
        contract, expected_audit=expected_audit_from_contract(contract)
    )
    markdown = render_chunk_repair_contract_markdown(contract)

    assert contract["schema_version"] == CHUNK_REPAIR_CONTRACT_VERSION
    assert contract["repair_targets"] == []
    assert contract["stable_id_counts"]["locator_count"] == 2
    assert contract["safety_boundary"]["import_eligible"] is False
    assert contract["safety_boundary"]["production_write_attempted"] is False
    assert result.passed is True
    assert validate_chunk_repair_contract_markdown(markdown) == []
    assert "```" not in markdown
    assert "raw_text" not in markdown
    assert "chunk_text" not in markdown


def test_audit_preflight_rejects_unsafe_or_drifted_audit_by_code_only() -> None:
    audit = _audit_fixture()
    # pyrefly: ignore [unsupported-operation]
    audit["safety_blockers"]["invariant_drift"] = ["/locator_count:expected=2:observed=3"]  # ty:ignore[invalid-assignment]
    audit["source_ledger_safety"] = {"raw_text": "DO NOT LEAK"}

    diagnostics = validate_locator_evidence_audit_for_repair_contract(audit)
    rendered = json.dumps([diagnostic.__dict__ for diagnostic in diagnostics])

    assert "audit_invariant_drift_present" in {diagnostic.code for diagnostic in diagnostics}
    assert "raw_text_leakage" in {diagnostic.code for diagnostic in diagnostics}
    assert "DO NOT LEAK" not in rendered


def test_renderer_cli_writes_temp_artifacts_that_validate(tmp_path: Path) -> None:
    audit_path = tmp_path / "audit.json"
    json_output = tmp_path / "chunk-repair-contract.json"
    markdown_output = tmp_path / "chunk-repair-contract.md"
    audit_path.write_text(json.dumps(_audit_fixture()), encoding="utf-8")

    exit_code = render_contract_main(
        [
            "--audit",
            str(audit_path),
            "--json-output",
            str(json_output),
            "--markdown-output",
            str(markdown_output),
        ]
    )

    contract = json.loads(json_output.read_text(encoding="utf-8"))
    markdown = markdown_output.read_text(encoding="utf-8")
    assert exit_code == 0
    assert (
        validate_chunk_repair_contract(
            contract, expected_audit=expected_audit_from_contract(contract)
        ).passed
        is True
    )
    assert validate_chunk_repair_contract_markdown(markdown) == []
    assert contract["diagnostics"]["production_import_attempted"] is False
    assert "No repair target is created" in markdown


def test_renderer_cli_rejects_invalid_audit_without_partial_writes(tmp_path: Path) -> None:
    audit_path = tmp_path / "bad-audit.json"
    json_output = tmp_path / "chunk-repair-contract.json"
    markdown_output = tmp_path / "chunk-repair-contract.md"
    bad_audit = _audit_fixture()
    # pyrefly: ignore [unsupported-operation]
    bad_audit["first_proof_invariants"]["locator_count"] = 99  # ty:ignore[invalid-assignment]
    audit_path.write_text(json.dumps(bad_audit), encoding="utf-8")

    exit_code = render_contract_main(
        [
            "--audit",
            str(audit_path),
            "--json-output",
            str(json_output),
            "--markdown-output",
            str(markdown_output),
        ]
    )

    assert exit_code == 2
    assert not json_output.exists()
    assert not markdown_output.exists()
