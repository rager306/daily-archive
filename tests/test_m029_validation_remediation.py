from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path
from typing import Any

import pytest

MODULE_PATH = Path(__file__).parents[1] / "scripts" / "verify_m029_validation_remediation.py"
spec = importlib.util.spec_from_file_location("verify_m029_validation_remediation", MODULE_PATH)
assert spec is not None and spec.loader is not None
verify_m029_validation_remediation = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = verify_m029_validation_remediation
spec.loader.exec_module(verify_m029_validation_remediation)

main = verify_m029_validation_remediation.main
validate_remediation = verify_m029_validation_remediation.validate_remediation
EXPECTED_REQUIREMENT_IDS = verify_m029_validation_remediation.EXPECTED_REQUIREMENT_IDS
REQUIRED_FALSE_FLAGS = verify_m029_validation_remediation.REQUIRED_FALSE_FLAGS


def _m029_selection() -> dict[str, Any]:
    return {
        "schema_version": "article-corpus-selection.v00.01",
        "selection_id": "m029-unified-corpus-v1",
        "articles": [
            {"article_ref": "arxiv/cs-cl/2507.19457", "identity_key": "arxiv:2507.19457"},
            {"article_ref": "arxiv/mixed-source/2605.26099", "identity_key": "arxiv:2605.26099"},
        ],
    }


def _m030_selection() -> dict[str, Any]:
    return {
        "schema_version": "article-corpus-selection.v00.02",
        "selection_id": "m029-pipeline-architecture-audit-v1",
        "refs": [
            {"ref_id": "m029-ref-001", "normalized_identity": "arxiv:2507.19457", "catalog_status": "already_cataloged", "prior_selection_status": "not_in_m028_selection", "source_kind": "arxiv_abs_url"},
            {"ref_id": "m029-ref-002", "normalized_identity": "stanford:cs224n:gradient-notes", "catalog_status": "missing_from_article_catalog", "prior_selection_status": "not_in_m028_selection", "source_kind": "external_pdf_url"},
            {"ref_id": "m029-ref-003", "normalized_identity": "arxiv:2605.29548", "catalog_status": "missing_from_article_catalog", "prior_selection_status": "not_in_m028_selection", "source_kind": "arxiv_abs_url"},
            {"ref_id": "m029-ref-004", "normalized_identity": "arxiv:2605.26099", "catalog_status": "missing_from_article_catalog", "prior_selection_status": "already_in_m028_selection", "source_kind": "arxiv_abs_url"},
        ],
    }


def _readiness() -> dict[str, Any]:
    return {
        "schema_version": "m029-unified-readiness-verifier.v1",
        "milestone_id": "M029-eb0ljz",
        "slice_id": "S06",
        "selection_id": "m029-unified-corpus-v1",
        "status": "passed",
        "article_count": 18,
        "ready_count": 11,
        "zero_chunk_count": 7,
        "unsafe_flag_count": 0,
        "decision": "partial_preprocessing_ready",
    }


def _requirement_rows() -> list[dict[str, Any]]:
    return [
        {
            "requirement_id": requirement_id,
            "coverage_status": "scoped_for_remediation_only",
            "validated": False,
            "validation_claim_allowed": False,
            "interpretation": "Current dossier narrows evidence boundaries only; validation remains blocked pending M030 completion and M029 replan proof.",
        }
        for requirement_id in EXPECTED_REQUIREMENT_IDS
    ]


def _bounded_rows() -> list[dict[str, Any]]:
    return [
        {
            "bounded_ref_id": "m029-ref-001",
            "normalized_identity": "arxiv:2507.19457",
            "present_in_provisional_m029_selection": True,
            "reconciliation_status": "represented_in_provisional_m029_corpus",
            "safe_next_action": "carry_forward_for_post_m030_replan_review",
            "m030_s01_catalog_status": "already_cataloged",
            "m030_s01_prior_selection_status": "not_in_m028_selection",
            "source_kind": "arxiv_abs_url",
        },
        {
            "bounded_ref_id": "m029-ref-002",
            "normalized_identity": "stanford:cs224n:gradient-notes",
            "present_in_provisional_m029_selection": False,
            "reconciliation_status": "missing_from_provisional_m029_corpus",
            "safe_next_action": "add_to_post_m030_replan_scope_before_validation",
            "m030_s01_catalog_status": "missing_from_article_catalog",
            "m030_s01_prior_selection_status": "not_in_m028_selection",
            "source_kind": "external_pdf_url",
        },
        {
            "bounded_ref_id": "m029-ref-003",
            "normalized_identity": "arxiv:2605.29548",
            "present_in_provisional_m029_selection": False,
            "reconciliation_status": "missing_from_provisional_m029_corpus",
            "safe_next_action": "add_to_post_m030_replan_scope_before_validation",
            "m030_s01_catalog_status": "missing_from_article_catalog",
            "m030_s01_prior_selection_status": "not_in_m028_selection",
            "source_kind": "arxiv_abs_url",
        },
        {
            "bounded_ref_id": "m029-ref-004",
            "normalized_identity": "arxiv:2605.26099",
            "present_in_provisional_m029_selection": True,
            "reconciliation_status": "represented_in_provisional_m029_corpus",
            "safe_next_action": "carry_forward_for_post_m030_replan_review",
            "m030_s01_catalog_status": "missing_from_article_catalog",
            "m030_s01_prior_selection_status": "already_in_m028_selection",
            "source_kind": "arxiv_abs_url",
        },
    ]


def _evidence() -> dict[str, Any]:
    return {
        "schema_version": "m029-validation-remediation-evidence.v1",
        "artifact_version": 1,
        "milestone_id": "M029-eb0ljz",
        "slice_id": "S07",
        "task_id": "T01",
        "selection_id": "m029-unified-corpus-v1",
        "created_at": "2026-06-04T00:00:00Z",
        "verdict": "blocked_pending_m030_completion",
        "verdict_reason": "M030 completion/S06 output and M030-derived M029 replan proof are absent.",
        "input_artifact_audit": [
            {"path": "data/article_corpora/m029-unified-corpus-v1/selection.json", "present": True},
            {"path": "data/article_corpora/m029-unified-corpus-v1/readiness-verify-summary.json", "present": True},
        ],
        "prerequisite_audit": {
            "status": "blocked_missing_m030_completion_and_s06_outputs",
            "m030_completion_required": True,
            "m030_completion_artifact": "artifacts/m030/MILESTONE-SUMMARY.md",
            "m030_completion_artifact_present": False,
            "m030_s06_roadmap_output_required": True,
            "m030_s06_summary": "artifacts/m030/S06-SUMMARY.md",
            "m030_s06_summary_present": False,
            "m030_s06_uat": "artifacts/m030/S06-UAT.md",
            "m030_s06_uat_present": False,
            "m030_s01_intake_summary": "artifacts/m030/S01-SUMMARY.md",
            "m030_s01_intake_summary_present": True,
        },
        "replan_audit": {
            "status": "blocked_missing_m030_derived_m029_replan_proof",
            "m029_replan_required_after_m030": True,
            "m030_derived_m029_replan_proof_present": False,
            "candidate_replan_artifacts": ["artifacts/m029/REPLAN.md", "artifacts/m029/ASSESSMENT.md"],
            "present_replan_artifacts": [],
        },
        "bounded_ref_reconciliation": _bounded_rows(),
        "bounded_ref_counts": {
            "m030_s01_bounded_ref_count": 4,
            "missing_from_provisional_m029_count": 2,
            "represented_in_provisional_m029_count": 2,
        },
        "provisional_m029_readiness_counts": _readiness() | {"interpretation": "Internal evidence only."},
        "requirement_coverage": _requirement_rows(),
        "metadata_only_boundary": {
            "relative_paths_only": True,
            "raw_article_text_included": False,
            "raw_pdf_bytes_included": False,
            "binary_payloads_included": False,
            "vectors_included": False,
            "secrets_included": False,
        },
        "safety_flags": dict.fromkeys(REQUIRED_FALSE_FLAGS, False),
        "safe_closeout_wording": [
            "M029 validation is blocked pending M030 completion and M030-derived M029 replan proof.",
            "No requirement is validated by this remediation dossier.",
        ],
        "forbidden_claims": ["M029 is validated.", "R024 is validated.", "LadybugDB was written."],
        "remaining_remediation_scope": ["Complete M030 and then replan M029."],
        "diagnostic_codes": [
            "M029_REMEDIATION_MISSING_BOUNDED_REF",
            "M029_REMEDIATION_MISSING_M029_REPLAN_PROOF",
            "M029_REMEDIATION_MISSING_M030_COMPLETION",
            "M029_REMEDIATION_MISSING_M030_S06_ROADMAP_OUTPUT",
        ],
        "diagnostic_count": 5,
    }


def _diagnostics_jsonl_rows() -> list[dict[str, Any]]:
    return [
        {"code": "M029_REMEDIATION_MISSING_M030_COMPLETION", "severity": "blocking", "json_path": "$.prerequisite_audit.m030_completion_artifact_present", "message": "M030 completion evidence is absent."},
        {"code": "M029_REMEDIATION_MISSING_M030_S06_ROADMAP_OUTPUT", "severity": "blocking", "json_path": "$.prerequisite_audit.m030_s06_summary_present", "message": "M030/S06 roadmap output evidence is absent."},
        {"code": "M029_REMEDIATION_MISSING_M029_REPLAN_PROOF", "severity": "blocking", "json_path": "$.replan_audit.m030_derived_m029_replan_proof_present", "message": "No M030-derived M029 replan proof artifact was found."},
        {"code": "M029_REMEDIATION_MISSING_BOUNDED_REF", "severity": "blocking", "json_path": "$.bounded_ref_reconciliation[1].present_in_provisional_m029_selection", "message": "Bounded ref is absent."},
        {"code": "M029_REMEDIATION_MISSING_BOUNDED_REF", "severity": "blocking", "json_path": "$.bounded_ref_reconciliation[2].present_in_provisional_m029_selection", "message": "Bounded ref is absent."},
    ]


def _report(evidence: dict[str, Any]) -> str:
    bounded_lines = "\n".join(
        f"| `{row['bounded_ref_id']}` | `{row['normalized_identity']}` | {str(row['present_in_provisional_m029_selection']).lower()} | `{row['reconciliation_status']}` |"
        for row in evidence["bounded_ref_reconciliation"]
    )
    requirement_lines = "\n".join(
        f"| `{row['requirement_id']}` | `{row['coverage_status']}` | {str(row['validated']).lower()} | {str(row['validation_claim_allowed']).lower()} |"
        for row in evidence["requirement_coverage"]
    )
    return f"""# M029 Validation Remediation Dossier

## Verdict

`{evidence['verdict']}`

## Prerequisite and Replan Status
M030 completion false; S06 summary false; M029 replan proof false.

## M030/S01 Bounded-Ref Reconciliation
| Ref | Identity | Present | Status |
|---|---|---:|---|
{bounded_lines}

## Provisional S06 Readiness
status passed; article_count 18; ready_count 11; zero_chunk_count 7; unsafe_flag_count 0; decision partial_preprocessing_ready.

## Requirement Coverage Narrowing
| Requirement | Coverage | Validated | Claim allowed |
|---|---|---:|---:|
{requirement_lines}
No requirement record was modified and no requirement is claimed validated by this dossier.

## Safety Flags
All unsafe flags remain false.

## Stable Diagnostics
M029_REMEDIATION_MISSING_M030_COMPLETION
M029_REMEDIATION_MISSING_M030_S06_ROADMAP_OUTPUT
M029_REMEDIATION_MISSING_M029_REPLAN_PROOF
M029_REMEDIATION_MISSING_BOUNDED_REF

## Safe Closeout Wording
M029 validation is blocked pending M030 completion and M030-derived M029 replan proof.

## Forbidden Claims
- M029 is validated.
- R024 is validated.
- LadybugDB was written.

## Remaining Remediation Scope
Complete M030 and then replan M029.
"""


def _fixture() -> tuple[dict[str, Any], str, list[dict[str, Any]], dict[str, Any], dict[str, Any], dict[str, Any]]:
    evidence = _evidence()
    return evidence, _report(evidence), _diagnostics_jsonl_rows(), _m029_selection(), _m030_selection(), _readiness()


def _errors(mutator=None) -> list[dict[str, Any]]:
    evidence, report, diagnostic_rows, m029_selection, m030_selection, readiness = _fixture()
    if mutator is not None:
        maybe_report = mutator(evidence, report, diagnostic_rows, m029_selection, m030_selection, readiness)
        if isinstance(maybe_report, str):
            report = maybe_report
    return validate_remediation(evidence, report, diagnostic_rows, m029_selection, m030_selection, readiness)


def _codes(errors: list[dict[str, Any]]) -> set[str]:
    return {str(error["code"]) for error in errors}


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def _write_fixture_files(tmp_path: Path) -> dict[str, Path]:
    evidence, report, diagnostic_rows, m029_selection, m030_selection, readiness = _fixture()
    paths = {
        "evidence": tmp_path / "evidence.json",
        "report": tmp_path / "report.md",
        "diagnostics": tmp_path / "diagnostics.jsonl",
        "m029_selection": tmp_path / "m029-selection.json",
        "m030_selection": tmp_path / "m030-selection.json",
        "readiness": tmp_path / "readiness.json",
        "summary": tmp_path / "verify-summary.json",
    }
    _write_json(paths["evidence"], evidence)
    paths["report"].write_text(report, encoding="utf-8")
    paths["diagnostics"].write_text("\n".join(json.dumps(row) for row in diagnostic_rows) + "\n", encoding="utf-8")
    _write_json(paths["m029_selection"], m029_selection)
    _write_json(paths["m030_selection"], m030_selection)
    _write_json(paths["readiness"], readiness)
    return paths


def _rel(path: Path, root: Path) -> str:
    return path.relative_to(root).as_posix()


def test_positive_fixture_passes_and_writes_metadata_summary(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    paths = _write_fixture_files(tmp_path)
    monkeypatch.chdir(tmp_path)

    exit_code = main([
        "--evidence", _rel(paths["evidence"], tmp_path),
        "--report", _rel(paths["report"], tmp_path),
        "--diagnostics", _rel(paths["diagnostics"], tmp_path),
        "--m029-selection", _rel(paths["m029_selection"], tmp_path),
        "--m030-selection", _rel(paths["m030_selection"], tmp_path),
        "--readiness-verify", _rel(paths["readiness"], tmp_path),
        "--write-verify-summary", _rel(paths["summary"], tmp_path),
        "--validate-only",
    ])

    assert exit_code == 0
    summary = json.loads(paths["summary"].read_text(encoding="utf-8"))
    assert summary["status"] == "passed"
    assert summary["verdict"] == "blocked_pending_m030_completion"
    assert summary["article_count"] == 18
    assert summary["present_bounded_refs"] == ["m029-ref-001", "m029-ref-004"]
    assert summary["missing_bounded_refs"] == ["m029-ref-002", "m029-ref-003"]
    assert summary["requirement_ids"] == list(EXPECTED_REQUIREMENT_IDS)
    assert summary["metadata_only"] is True
    assert summary["network_fetch_attempted"] is False


@pytest.mark.parametrize(
    "name,mutator,expected_code",
    [
        (
            "false pass verdict",
            lambda evidence, *_: evidence.__setitem__("verdict", "passed"),
            "M029_REMEDIATION_FALSE_PASS_VERDICT",
        ),
        (
            "missing M030 completion evidence overclaim",
            lambda evidence, *_: evidence["prerequisite_audit"].__setitem__("m030_completion_artifact_present", True),
            "M029_REMEDIATION_PREREQUISITE_OVERCLAIM",
        ),
        (
            "missing bounded-ref row",
            lambda evidence, *_: evidence["bounded_ref_reconciliation"].pop(),
            "M029_REMEDIATION_BOUNDED_REF_MISMATCH",
        ),
        (
            "readiness count drift",
            lambda evidence, *_: evidence["provisional_m029_readiness_counts"].__setitem__("ready_count", 12),
            "M029_REMEDIATION_READINESS_COUNT_DRIFT",
        ),
        (
            "unsafe true flag",
            lambda evidence, *_: evidence["safety_flags"].__setitem__("ladybugdb_written", True),
            "M029_REMEDIATION_UNSAFE_FLAG_TRUE",
        ),
        (
            "raw payload field name",
            lambda evidence, *_: evidence.__setitem__("raw_article_text", "payload must never appear"),
            "M029_REMEDIATION_RAW_PAYLOAD_FIELD",
        ),
        (
            "requirement validated overclaim",
            lambda evidence, *_: evidence["requirement_coverage"][0].__setitem__("validated", True),
            "M029_REMEDIATION_REQUIREMENT_STATUS_OVERCLAIM",
        ),
    ],
)
def test_negative_boundaries_fail_closed(name: str, mutator, expected_code: str) -> None:
    errors = _errors(mutator)
    assert expected_code in _codes(errors), name


def test_forbidden_report_phrase_fails_outside_forbidden_claims_section() -> None:
    def mutate(_evidence: dict[str, Any], report: str, *_args: Any) -> str:
        return report.replace("## Safety Flags", "M029 is validated.\n\n## Safety Flags")

    assert "M029_REMEDIATION_FORBIDDEN_POSITIVE_CLAIM" in _codes(_errors(mutate))


def test_forbidden_claims_section_itself_is_allowed_for_dossier_inventory() -> None:
    assert _errors() == []


def test_missing_required_diagnostic_row_fails_closed() -> None:
    def mutate(evidence: dict[str, Any], _report: str, diagnostic_rows: list[dict[str, Any]], *_args: Any) -> None:
        evidence["diagnostic_count"] = 4
        diagnostic_rows[:] = [row for row in diagnostic_rows if row["code"] != "M029_REMEDIATION_MISSING_M029_REPLAN_PROOF"]
        evidence["diagnostic_codes"] = sorted({row["code"] for row in diagnostic_rows})
        return None

    assert "M029_REMEDIATION_DIAGNOSTIC_DRIFT" in _codes(_errors(mutate))


def test_malformed_jsonl_returns_nonzero_with_line_context(tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]) -> None:
    paths = _write_fixture_files(tmp_path)
    paths["diagnostics"].write_text('{"code":"ok"}\nnot-json\n', encoding="utf-8")
    monkeypatch.chdir(tmp_path)

    exit_code = main([
        "--evidence", _rel(paths["evidence"], tmp_path),
        "--report", _rel(paths["report"], tmp_path),
        "--diagnostics", _rel(paths["diagnostics"], tmp_path),
        "--m029-selection", _rel(paths["m029_selection"], tmp_path),
        "--m030-selection", _rel(paths["m030_selection"], tmp_path),
        "--readiness-verify", _rel(paths["readiness"], tmp_path),
    ])

    assert exit_code == 2
    assert "diagnostics.jsonl:2" in capsys.readouterr().err


def test_absolute_input_path_is_rejected(tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]) -> None:
    paths = _write_fixture_files(tmp_path)
    monkeypatch.chdir(tmp_path)

    exit_code = main([
        "--evidence", str(paths["evidence"].resolve()),
        "--report", _rel(paths["report"], tmp_path),
        "--diagnostics", _rel(paths["diagnostics"], tmp_path),
        "--m029-selection", _rel(paths["m029_selection"], tmp_path),
        "--m030-selection", _rel(paths["m030_selection"], tmp_path),
        "--readiness-verify", _rel(paths["readiness"], tmp_path),
    ])

    assert exit_code == 2
    assert "repo-relative" in capsys.readouterr().err


def test_real_t01_artifacts_validate_in_repo_when_present() -> None:
    root = Path(__file__).parents[1]
    required = [
        root / "data/article_corpora/m029-unified-corpus-v1/validation-remediation/remediation-evidence.json",
        root / "data/article_corpora/m029-unified-corpus-v1/validation-remediation/remediation-report.md",
        root / "data/article_corpora/m029-unified-corpus-v1/validation-remediation/remediation-diagnostics.jsonl",
        root / "data/article_corpora/m029-unified-corpus-v1/selection.json",
        root / "data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json",
        root / "data/article_corpora/m029-unified-corpus-v1/readiness-verify-summary.json",
    ]
    if not all(path.exists() for path in required):
        pytest.skip("M029 T01 artifacts are not available in this checkout")

    evidence = json.loads(required[0].read_text(encoding="utf-8"))
    report = required[1].read_text(encoding="utf-8")
    diagnostic_rows = [json.loads(line) for line in required[2].read_text(encoding="utf-8").splitlines() if line.strip()]
    m029_selection = json.loads(required[3].read_text(encoding="utf-8"))
    m030_selection = json.loads(required[4].read_text(encoding="utf-8"))
    readiness = json.loads(required[5].read_text(encoding="utf-8"))

    assert validate_remediation(evidence, report, diagnostic_rows, m029_selection, m030_selection, readiness) == []
