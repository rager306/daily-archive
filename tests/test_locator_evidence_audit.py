from __future__ import annotations

import copy
import json
import subprocess
import sys
from pathlib import Path

import pytest

from scripts.audit_locator_evidence import (
    LocatorEvidenceAuditError,
    audit_locator_evidence,
    audit_locator_evidence_file,
    load_locator_artifact,
)

FIXTURE = Path("tests/fixtures/locator_evidence_audit_batch.json")
FIXTURE_EXPECTED = {
    "schema_version": "candidate_locator_protocol.v1",
    "paper_count": 2,
    "source_count": 2,
    "locator_count": 3,
    "located_count": 3,
    "review_required_count": 1,
    "missing_span_count": 0,
    "ambiguous_span_count": 1,
    "conflicting_evidence_count": 0,
    "retrieval_only_count": 1,
    "repair_required_count": 0,
    "import_eligible_count": 0,
    "promoted_to_fact_count": 0,
}


def _fixture() -> dict:
    return json.loads(FIXTURE.read_text(encoding="utf-8"))


def test_audit_reports_redacted_counts_distributions_and_coverage() -> None:
    audit = audit_locator_evidence(_fixture(), expected_invariants=FIXTURE_EXPECTED)

    assert audit["schema_version"] == "locator_evidence_audit.v1"
    assert audit["input_schema_version"] == "candidate_locator_protocol.v1"
    assert audit["first_proof_invariants"] == FIXTURE_EXPECTED
    assert audit["distributions"]["states"] == {
        "ambiguous_span": 1,
        "retrieval_only": 1,
        "review_required": 1,
    }
    assert audit["distributions"]["routes"] == {
        "claim_location": 1,
        "method_location": 1,
        "retrieval_context": 1,
    }
    assert audit["diagnostic_code_classes"] == {"ambiguous_span": 1, "review_required": 2}
    assert audit["source_span_coverage"]["locators_with_source_spans"] == 3
    assert audit["source_span_coverage"]["coordinate_spans_with_char_bounds"] == 3
    assert audit["source_span_coverage"]["spans_with_hash"] == 3
    assert audit["source_ledger_safety"]["source_text_embedded_nonfalse_paths"] == []
    assert audit["repair_context_gaps"]["missing_span_locator_ids"] == []
    assert audit["safety_blockers"]["no_import_blocker_intact"] is True
    assert "source_path" not in json.dumps(audit)
    assert "raw_text" not in json.dumps(audit)


def test_file_helper_writes_output_only_after_success(tmp_path: Path) -> None:
    output = tmp_path / "audit.json"

    audit = audit_locator_evidence_file(
        FIXTURE,
        output_path=output,
        expected_invariants=FIXTURE_EXPECTED,
    )

    assert output.exists()
    assert json.loads(output.read_text(encoding="utf-8")) == audit


def test_cli_non_strict_is_documented_entrypoint(tmp_path: Path) -> None:
    output = tmp_path / "audit.json"

    result = subprocess.run(
        [
            sys.executable,
            "scripts/audit_locator_evidence.py",
            str(FIXTURE),
            "--non-strict",
            "--output",
            str(output),
        ],
        check=False,
        text=True,
        capture_output=True,
    )

    assert result.returncode == 0
    persisted = json.loads(output.read_text(encoding="utf-8"))
    assert persisted["strict"] is False
    assert persisted["first_proof_invariants"]["locator_count"] == 3


def test_cli_writes_json_and_markdown_outputs(tmp_path: Path) -> None:
    json_output = tmp_path / "audit.json"
    markdown_output = tmp_path / "audit.md"

    result = subprocess.run(
        [
            sys.executable,
            "scripts/audit_locator_evidence.py",
            str(FIXTURE),
            "--json-output",
            str(json_output),
            "--markdown-output",
            str(markdown_output),
            "--non-strict",
        ],
        check=False,
        text=True,
        capture_output=True,
    )

    assert result.returncode == 0
    persisted = json.loads(json_output.read_text(encoding="utf-8"))
    markdown = markdown_output.read_text(encoding="utf-8")
    assert persisted["input_path"] == str(FIXTURE)
    assert persisted["strict"] is False
    assert "# S01 Locator Evidence Audit" in markdown
    assert "Expected First-Proof Invariants" in markdown
    assert "Explicit No-Go Constraints" in markdown
    assert "raw_text" not in markdown
    assert "source_path" not in markdown


def test_missing_input_fails_with_clear_path_error(tmp_path: Path) -> None:
    missing = tmp_path / "missing.json"

    with pytest.raises(FileNotFoundError, match="locator evidence input file not found"):
        load_locator_artifact(missing)


def test_malformed_json_fails_before_output_write(tmp_path: Path) -> None:
    bad = tmp_path / "bad.json"
    bad.write_text("{not-json", encoding="utf-8")
    output = tmp_path / "audit.json"

    with pytest.raises(LocatorEvidenceAuditError, match="malformed locator evidence JSON"):
        audit_locator_evidence_file(bad, output_path=output, strict=False)

    assert not output.exists()


def test_wrong_schema_surfaces_validator_code_only() -> None:
    artifact = _fixture()
    artifact["schema_version"] = "wrong"

    with pytest.raises(LocatorEvidenceAuditError, match="invalid_schema_version"):
        audit_locator_evidence(artifact, strict=False)


def test_missing_source_spans_fail_via_validator_path_code() -> None:
    artifact = _fixture()
    artifact["locators"][0]["source_spans"] = []

    with pytest.raises(
        LocatorEvidenceAuditError, match="missing_source_spans:m021-paper-1-claim-001"
    ):
        audit_locator_evidence(artifact, strict=False)


def test_forbidden_payload_key_injection_is_rejected_without_value() -> None:
    artifact = _fixture()
    artifact["locators"][0]["source_spans"][0]["chunk_text"] = "do not leak this value"

    with pytest.raises(LocatorEvidenceAuditError) as exc_info:
        audit_locator_evidence(artifact, strict=False)

    message = str(exc_info.value)
    assert "/chunk_text" in message
    assert "do not leak this value" not in message


def test_changed_invariant_counts_fail_in_strict_mode() -> None:
    artifact = _fixture()
    artifact["locators"] = artifact["locators"][:-1]
    artifact["summary"]["locator_count"] = 2
    artifact["summary"]["located_count"] = 2
    artifact["summary"]["review_required_count"] = 0
    artifact["summary"]["source_count"] = 2

    with pytest.raises(LocatorEvidenceAuditError, match="invariant drift"):
        audit_locator_evidence(artifact, expected_invariants=FIXTURE_EXPECTED)


def test_non_strict_allows_count_drift_but_keeps_safety_checks() -> None:
    artifact = _fixture()
    artifact["locators"] = artifact["locators"][:1]
    artifact["summary"]["locator_count"] = 1
    artifact["summary"]["located_count"] = 1
    artifact["summary"]["retrieval_only_count"] = 0
    artifact["summary"]["review_required_count"] = 0
    artifact["summary"]["paper_count"] = 1

    audit = audit_locator_evidence(artifact, strict=False)

    assert audit["first_proof_invariants"]["locator_count"] == 1
    assert audit["safety_blockers"]["invariant_drift"] == []


def test_non_false_safety_flags_are_rejected() -> None:
    artifact = _fixture()
    artifact["safety_flags"]["ladybugdb_written"] = True

    with pytest.raises(LocatorEvidenceAuditError, match="safety_flag_true:ladybugdb_written"):
        audit_locator_evidence(artifact, strict=False)


def test_summary_drift_is_rejected_even_when_non_strict() -> None:
    artifact = copy.deepcopy(_fixture())
    artifact["summary"]["locator_count"] = 99

    with pytest.raises(LocatorEvidenceAuditError, match="summary drift"):
        audit_locator_evidence(artifact, strict=False)
