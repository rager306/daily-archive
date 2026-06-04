from __future__ import annotations

import importlib.util
import json
import sys
from copy import deepcopy
from pathlib import Path
from typing import Any

import pytest

MODULE_PATH = Path(__file__).parents[1] / "scripts" / "verify_m029_post_validation_remediation.py"
spec = importlib.util.spec_from_file_location("verify_m029_post_validation_remediation", MODULE_PATH)
assert spec is not None and spec.loader is not None
verify_m029_post_validation_remediation = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = verify_m029_post_validation_remediation
spec.loader.exec_module(verify_m029_post_validation_remediation)

main = verify_m029_post_validation_remediation.main
validate_remediation = verify_m029_post_validation_remediation.validate_remediation
EXPECTED_IN_SCOPE_REQUIREMENTS = verify_m029_post_validation_remediation.EXPECTED_IN_SCOPE_REQUIREMENTS
EXPECTED_OUT_OF_SCOPE_REQUIREMENTS = verify_m029_post_validation_remediation.EXPECTED_OUT_OF_SCOPE_REQUIREMENTS
REQUIRED_FALSE_SAFETY_FLAGS = verify_m029_post_validation_remediation.REQUIRED_FALSE_SAFETY_FLAGS
REQUIRED_FALSE_BOUNDARY_FLAGS = verify_m029_post_validation_remediation.REQUIRED_FALSE_BOUNDARY_FLAGS
REQUIRED_DIAGNOSTIC_CODES = verify_m029_post_validation_remediation.REQUIRED_DIAGNOSTIC_CODES


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
            {"ref_id": "m029-ref-004", "normalized_identity": "arxiv:2605.26099", "catalog_status": "already_cataloged", "prior_selection_status": "already_in_m028_selection", "source_kind": "arxiv_abs_url"},
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


def _s07_summary() -> dict[str, Any]:
    return {
        "schema_version": "m029-validation-remediation-verifier.v1",
        "milestone_id": "M029-eb0ljz",
        "slice_id": "S07",
        "selection_id": "m029-unified-corpus-v1",
        "status": "passed",
        "verdict": "blocked_pending_m030_completion",
        "article_count": 18,
        "ready_count": 11,
        "zero_chunk_count": 7,
        "unsafe_flag_count": 0,
        "present_bounded_refs": ["m029-ref-001", "m029-ref-004"],
        "missing_bounded_refs": ["m029-ref-002", "m029-ref-003"],
    }


def _bounded_rows() -> list[dict[str, Any]]:
    return [
        {
            "bounded_ref_id": "m029-ref-001",
            "normalized_identity": "arxiv:2507.19457",
            "source_kind": "arxiv_abs_url",
            "m030_s01_catalog_status": "already_cataloged",
            "m030_s01_prior_selection_status": "not_in_m028_selection",
            "present_in_provisional_m029_selection": True,
            "reconciliation_status": "represented_in_provisional_m029_corpus",
            "safe_next_action": "carry_forward_for_post_m030_replan_review",
        },
        {
            "bounded_ref_id": "m029-ref-002",
            "normalized_identity": "stanford:cs224n:gradient-notes",
            "source_kind": "external_pdf_url",
            "m030_s01_catalog_status": "missing_from_article_catalog",
            "m030_s01_prior_selection_status": "not_in_m028_selection",
            "present_in_provisional_m029_selection": False,
            "reconciliation_status": "missing_from_provisional_m029_corpus",
            "safe_next_action": "redo_or_explicitly_descoped_in_post_m030_replan_before_validation",
        },
        {
            "bounded_ref_id": "m029-ref-003",
            "normalized_identity": "arxiv:2605.29548",
            "source_kind": "arxiv_abs_url",
            "m030_s01_catalog_status": "missing_from_article_catalog",
            "m030_s01_prior_selection_status": "not_in_m028_selection",
            "present_in_provisional_m029_selection": False,
            "reconciliation_status": "missing_from_provisional_m029_corpus",
            "safe_next_action": "redo_or_explicitly_descoped_in_post_m030_replan_before_validation",
        },
        {
            "bounded_ref_id": "m029-ref-004",
            "normalized_identity": "arxiv:2605.26099",
            "source_kind": "arxiv_abs_url",
            "m030_s01_catalog_status": "already_cataloged",
            "m030_s01_prior_selection_status": "already_in_m028_selection",
            "present_in_provisional_m029_selection": True,
            "reconciliation_status": "represented_in_provisional_m029_corpus",
            "safe_next_action": "carry_forward_for_post_m030_replan_review",
        },
    ]


def _requirement_scope() -> dict[str, Any]:
    return {
        "in_scope_m029_remediation_requirements": [
            {
                "requirement_id": requirement_id,
                "coverage_status": "advanced_not_validated",
                "validated": False,
                "validation_claim_allowed": False,
                "interpretation": "Scoped to remediation evidence only; blocked until M030 completion and M030-derived M029 replan proof exist.",
            }
            for requirement_id in EXPECTED_IN_SCOPE_REQUIREMENTS
        ],
        "out_of_scope_project_requirements": [
            {
                "requirement_id": requirement_id,
                "scope_status": "out_of_scope_for_m029_post_validation_remediation",
                "advanced": False,
                "validated": False,
            }
            for requirement_id in EXPECTED_OUT_OF_SCOPE_REQUIREMENTS
        ],
        "requirement_records_modified": False,
        "validated_requirement_count": 0,
    }


def _evidence() -> dict[str, Any]:
    return {
        "schema_version": "m029-post-validation-remediation-evidence.v1",
        "artifact_version": 1,
        "milestone_id": "M029-eb0ljz",
        "slice_id": "S08",
        "task_id": "T01",
        "selection_id": "m029-unified-corpus-v1",
        "created_at": "2026-06-04T00:00:00Z",
        "verdict": "blocked_pending_m030_completion_and_replan",
        "verdict_reason": "M030/S02-S06 and M030-derived M029 replan proof are absent.",
        "metadata_only_boundary": {"relative_paths_only": True, **{key: False for key in REQUIRED_FALSE_BOUNDARY_FLAGS}},
        "source_artifact_paths": {
            "m029_roadmap": "fixtures/M029-ROADMAP.md",
            "m029_s07_summary": "fixtures/S07-SUMMARY.md",
            "m030_roadmap": "fixtures/M030-ROADMAP.md",
            "m030_s01_summary": "fixtures/S01-SUMMARY.md",
            "s07_remediation_evidence": "fixtures/s07-evidence.json",
            "s07_remediation_verify_summary": "fixtures/s07-summary.json",
            "m029_readiness_verify_summary": "fixtures/readiness.json",
            "m029_selection": "fixtures/m029-selection.json",
            "m030_s01_selection": "fixtures/m030-selection.json",
        },
        "input_artifact_audit": [
            {"path": "fixtures/M030-S02-SUMMARY.md", "present": False, "role": "M030/S02 absent."},
            {"path": "fixtures/M030-S03-SUMMARY.md", "present": False, "role": "M030/S03 absent."},
        ],
        "prerequisite_audit": {
            "status": "blocked_missing_m030_s02_s06_and_milestone_completion",
            "m030_milestone_id": "M030-abwhdm",
            "m030_completion_required": True,
            "m030_completion_artifact": "fixtures/MILESTONE-SUMMARY.md",
            "m030_completion_artifact_present": False,
            "slices": [
                {"slice_id": "S01", "title": "Requested Ref Intake", "status": "complete", "evidence_path": "fixtures/S01-SUMMARY.md", "evidence_present": True},
                {"slice_id": "S02", "title": "Code Module Inventory", "status": "pending", "evidence_path": "fixtures/S02-SUMMARY.md", "evidence_present": False},
                {"slice_id": "S03", "title": "Module Function Readiness Matrix", "status": "pending", "evidence_path": "fixtures/S03-SUMMARY.md", "evidence_present": False},
                {"slice_id": "S04", "title": "Requirement to Module Coverage Matrix", "status": "pending", "evidence_path": "fixtures/S04-SUMMARY.md", "evidence_present": False},
                {"slice_id": "S05", "title": "End to End Process Continuity Audit", "status": "pending", "evidence_path": "fixtures/S05-SUMMARY.md", "evidence_present": False},
                {"slice_id": "S06", "title": "Implementation Roadmap from Audit", "status": "pending", "evidence_path": "fixtures/S06-SUMMARY.md", "evidence_present": False},
            ],
        },
        "m030_derived_m029_replan_audit": {
            "status": "blocked_missing_m030_derived_m029_replan_proof",
            "m029_replan_required_after_m030": True,
            "m030_s06_roadmap_output_required": True,
            "m030_s06_roadmap_output_path": "fixtures/m030_next_implementation_roadmap.json",
            "m030_s06_roadmap_output_present": False,
            "candidate_replan_artifacts": [
                {"path": "fixtures/REPLAN.md", "present": False},
                {"path": "fixtures/ASSESSMENT.md", "present": False},
            ],
        },
        "bounded_ref_counts": {
            "m030_s01_bounded_ref_count": 4,
            "represented_in_provisional_m029_count": 2,
            "missing_from_provisional_m029_count": 2,
        },
        "bounded_ref_reconciliation": _bounded_rows(),
        "provisional_m029_readiness_counts": _readiness() | {"interpretation": "Local evidence only."},
        "requirement_scope": _requirement_scope(),
        "safety_flags": {key: False for key in REQUIRED_FALSE_SAFETY_FLAGS},
        "forbidden_claims": [
            "M029 validation passed.",
            "M029 is production ready.",
            "M029 is ready for graph import.",
            "M029 is ready for KG import.",
            "Any M029 remediation requirement is validated.",
            "LadybugDB was written.",
            "Production import was attempted.",
        ],
        "safe_closeout_wording": [
            "M029 remains blocked pending M030/S02-S06 completion, M030 milestone closeout, and M030-derived M029 replan proof.",
            "R024, R027, R029, R035, R040, and R050 are advanced only within the remediation boundary and are not validated.",
        ],
        "remaining_remediation_scope": ["Complete M030/S02-S06 before M029 validation."],
        "diagnostic_codes": sorted(REQUIRED_DIAGNOSTIC_CODES),
        "diagnostic_count": 10,
        "observability": {
            "diagnostics_path": "fixtures/post-validation-remediation-diagnostics.jsonl",
            "expected_future_verify_summary_path": "fixtures/post-validation-remediation-verify-summary.json",
            "local_only": True,
            "dashboard": False,
            "pager": False,
            "network_check": False,
            "runtime_service": False,
            "production_monitoring_surface": False,
        },
    }


def _s07_evidence() -> dict[str, Any]:
    return {
        "schema_version": "m029-validation-remediation-evidence.v1",
        "slice_id": "S07",
        "bounded_ref_counts": deepcopy(_evidence()["bounded_ref_counts"]),
        "bounded_ref_reconciliation": deepcopy(_bounded_rows()),
    }


def _diagnostics_jsonl_rows() -> list[dict[str, Any]]:
    rows = [
        ("M029_POST_VALIDATION_MISSING_M030_COMPLETION", "$.prerequisite_audit.m030_completion_artifact_present"),
        ("M029_POST_VALIDATION_PENDING_M030_S02", "$.prerequisite_audit.slices[1].evidence_present"),
        ("M029_POST_VALIDATION_PENDING_M030_S03", "$.prerequisite_audit.slices[2].evidence_present"),
        ("M029_POST_VALIDATION_PENDING_M030_S04", "$.prerequisite_audit.slices[3].evidence_present"),
        ("M029_POST_VALIDATION_PENDING_M030_S05", "$.prerequisite_audit.slices[4].evidence_present"),
        ("M029_POST_VALIDATION_PENDING_M030_S06", "$.prerequisite_audit.slices[5].evidence_present"),
        ("M029_POST_VALIDATION_MISSING_M030_S06_OUTPUT", "$.m030_derived_m029_replan_audit.m030_s06_roadmap_output_present"),
        ("M029_POST_VALIDATION_MISSING_M029_REPLAN_PROOF", "$.m030_derived_m029_replan_audit.candidate_replan_artifacts"),
        ("M029_POST_VALIDATION_MISSING_BOUNDED_REF", "$.bounded_ref_reconciliation[1].present_in_provisional_m029_selection"),
        ("M029_POST_VALIDATION_MISSING_BOUNDED_REF", "$.bounded_ref_reconciliation[2].present_in_provisional_m029_selection"),
    ]
    return [{"code": code, "severity": "blocker", "json_path": path, "message": f"{code} remains blocking."} for code, path in rows]


def _report(evidence: dict[str, Any]) -> str:
    bounded_lines = "\n".join(
        f"| `{row['bounded_ref_id']}` | `{row['normalized_identity']}` | {str(row['present_in_provisional_m029_selection']).lower()} | `{row['reconciliation_status']}` |"
        for row in evidence["bounded_ref_reconciliation"]
    )
    requirement_lines = "\n".join(
        f"| {row['requirement_id']} | advanced not validated | no |" for row in evidence["requirement_scope"]["in_scope_m029_remediation_requirements"]
    )
    return f"""# M029 Post-Validation Remediation Closure Report

## Verdict

`{evidence['verdict']}`

M029 still cannot close. The post-validation dossier is metadata-only evidence for re-validation planning; it is not validation, production readiness, graph/KG readiness, import readiness, or requirement validation.

## Prerequisite Audit
M030/S02-S06 remain pending and M030 milestone completion is absent.

## M030-Derived M029 Replan Audit
M030/S06 roadmap output and M030-derived M029 replan proof are absent.

## Bounded-Ref Reconciliation
| Ref | Identity | Present | Status |
|---|---|---:|---|
{bounded_lines}
Bounded-ref counts: four total, two represented, two missing.

## Provisional M029 Readiness Context
status passed; article_count 18; ready_count 11; zero_chunk_count 7; unsafe_flag_count 0; decision partial_preprocessing_ready.

## In-Scope M029 Requirement Coverage
| Requirement | Status | Validated? |
|---|---|---:|
{requirement_lines}
Validated requirement count: 0.

## Out-of-Scope Project Requirements
R019, R022, R023, R031, R032, R033, R051, and R052 are out of scope for M029 post-validation remediation.

## Advanced-Not-Validated Requirements
R024, R027, R029, R035, R040, and R050 are advanced only in the narrow fail-closed remediation sense.

## Safety Flags
All unsafe flags are false.

## Forbidden Claims
- M029 validation passed.
- M029 is production ready.
- M029 is ready for graph import.
- Any M029 remediation requirement is validated.
- LadybugDB was written.
- Production import was attempted.

## Remaining Remediation Scope
Complete M030/S02-S06 and then replan M029.

## Failure Modes (Q5)
Local JSON/Markdown/JSONL filesystem inputs can be missing or malformed; verifier returns non-zero with stable diagnostics or input errors. No network/API dependency exists.

## Load Profile (Q6)
Linear scan of small metadata files; at 10x size local JSON/Markdown parsing saturates first. No runtime service, database pool, or network load dimension exists.

## Negative Tests (Q7)
Tests cover malformed JSONL, absolute paths, path escapes, unsafe true flags/raw payload fields, positive claims, missing bounded-ref diagnostics, bounded-ref count mismatch, and requirement validation overclaims.

## Observability Impact
Failure visibility is local-only through diagnostics JSONL and verify-summary JSON. No dashboard, pager, network check, runtime service, or production monitoring surface exists.
"""


def _fixture() -> tuple[dict[str, Any], str, list[dict[str, Any]], dict[str, Any], dict[str, Any], dict[str, Any], dict[str, Any], dict[str, Any]]:
    evidence = _evidence()
    return evidence, _report(evidence), _diagnostics_jsonl_rows(), _s07_evidence(), _s07_summary(), _m029_selection(), _readiness(), _m030_selection()


def _errors(mutator=None) -> list[dict[str, Any]]:
    evidence, report, diagnostic_rows, s07_evidence, s07_summary, m029_selection, readiness, m030_selection = _fixture()
    if mutator is not None:
        maybe_report = mutator(evidence, report, diagnostic_rows, s07_evidence, s07_summary, m029_selection, readiness, m030_selection)
        if isinstance(maybe_report, str):
            report = maybe_report
    return validate_remediation(evidence, report, diagnostic_rows, s07_evidence, s07_summary, m029_selection, readiness, m030_selection)


def _codes(errors: list[dict[str, Any]]) -> set[str]:
    return {str(error["code"]) for error in errors}


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def _write_fixture_files(tmp_path: Path) -> dict[str, Path]:
    evidence, report, diagnostic_rows, s07_evidence, s07_summary, m029_selection, readiness, m030_selection = _fixture()
    paths = {
        "evidence": tmp_path / "evidence.json",
        "report": tmp_path / "report.md",
        "diagnostics": tmp_path / "diagnostics.jsonl",
        "s07_evidence": tmp_path / "s07-evidence.json",
        "s07_summary": tmp_path / "s07-summary.json",
        "m029_selection": tmp_path / "m029-selection.json",
        "readiness": tmp_path / "readiness.json",
        "m030_selection": tmp_path / "m030-selection.json",
        "summary": tmp_path / "verify-summary.json",
    }
    _write_json(paths["evidence"], evidence)
    paths["report"].write_text(report, encoding="utf-8")
    paths["diagnostics"].write_text("\n".join(json.dumps(row) for row in diagnostic_rows) + "\n", encoding="utf-8")
    _write_json(paths["s07_evidence"], s07_evidence)
    _write_json(paths["s07_summary"], s07_summary)
    _write_json(paths["m029_selection"], m029_selection)
    _write_json(paths["readiness"], readiness)
    _write_json(paths["m030_selection"], m030_selection)
    return paths


def _rel(path: Path, root: Path) -> str:
    return path.relative_to(root).as_posix()


def _valid_argv(paths: dict[str, Path], root: Path) -> list[str]:
    return [
        "--evidence", _rel(paths["evidence"], root),
        "--report", _rel(paths["report"], root),
        "--diagnostics", _rel(paths["diagnostics"], root),
        "--s07-evidence", _rel(paths["s07_evidence"], root),
        "--s07-verify-summary", _rel(paths["s07_summary"], root),
        "--m029-selection", _rel(paths["m029_selection"], root),
        "--m029-readiness-summary", _rel(paths["readiness"], root),
        "--m030-requested-ref-selection", _rel(paths["m030_selection"], root),
    ]


def test_positive_fixture_passes_and_writes_metadata_summary(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    paths = _write_fixture_files(tmp_path)
    monkeypatch.chdir(tmp_path)

    exit_code = main([*_valid_argv(paths, tmp_path), "--write-verify-summary", _rel(paths["summary"], tmp_path), "--validate-only"])

    assert exit_code == 0
    summary = json.loads(paths["summary"].read_text(encoding="utf-8"))
    assert summary["status"] == "passed"
    assert summary["blocked_verdict"] is True
    assert summary["verdict"] == "blocked_pending_m030_completion_and_replan"
    assert summary["present_bounded_refs"] == ["m029-ref-001", "m029-ref-004"]
    assert summary["missing_bounded_refs"] == ["m029-ref-002", "m029-ref-003"]
    assert summary["in_scope_requirement_ids"] == list(EXPECTED_IN_SCOPE_REQUIREMENTS)
    assert summary["out_of_scope_requirement_ids"] == list(EXPECTED_OUT_OF_SCOPE_REQUIREMENTS)
    assert summary["validated_requirement_count"] == 0
    assert summary["metadata_only"] is True
    assert summary["network_fetch_attempted"] is False
    assert summary["diagnostic_count"] == 0


@pytest.mark.parametrize(
    "name,mutator,expected_code",
    [
        (
            "false pass verdict",
            lambda evidence, *_: evidence.__setitem__("verdict", "passed"),
            "M029_POST_VALIDATION_FALSE_PASS_VERDICT",
        ),
        (
            "positive prerequisite overclaim",
            lambda evidence, *_: evidence["prerequisite_audit"].__setitem__("m030_completion_artifact_present", True),
            "M029_POST_VALIDATION_PREREQUISITE_OVERCLAIM",
        ),
        (
            "path escape",
            lambda evidence, *_: evidence["input_artifact_audit"][0].__setitem__("path", "../outside.json"),
            "M029_POST_VALIDATION_UNSAFE_SOURCE_PATH",
        ),
        (
            "unsafe true flag",
            lambda evidence, *_: evidence["safety_flags"].__setitem__("ladybugdb_written", True),
            "M029_POST_VALIDATION_UNSAFE_FLAG_TRUE",
        ),
        (
            "raw payload field name",
            lambda evidence, *_: evidence.__setitem__("raw_article_text", "payload must never appear"),
            "M029_POST_VALIDATION_RAW_PAYLOAD_FIELD",
        ),
        (
            "missing bounded ref row",
            lambda evidence, *_: evidence["bounded_ref_reconciliation"].pop(),
            "M029_POST_VALIDATION_BOUNDED_REF_MISMATCH",
        ),
        (
            "bounded ref count mismatch",
            lambda evidence, *_: evidence["bounded_ref_counts"].__setitem__("missing_from_provisional_m029_count", 1),
            "M029_POST_VALIDATION_BOUNDED_REF_COUNT_DRIFT",
        ),
        (
            "requirement validation overclaim",
            lambda evidence, *_: evidence["requirement_scope"]["in_scope_m029_remediation_requirements"][0].__setitem__("validated", True),
            "M029_POST_VALIDATION_REQUIREMENT_STATUS_OVERCLAIM",
        ),
        (
            "out of scope requirement advanced",
            lambda evidence, *_: evidence["requirement_scope"]["out_of_scope_project_requirements"][0].__setitem__("advanced", True),
            "M029_POST_VALIDATION_REQUIREMENT_STATUS_OVERCLAIM",
        ),
    ],
)
def test_negative_boundaries_fail_closed(name: str, mutator, expected_code: str) -> None:
    errors = _errors(mutator)
    assert expected_code in _codes(errors), name


def test_forbidden_report_phrase_fails_outside_forbidden_claims_section() -> None:
    def mutate(_evidence: dict[str, Any], report: str, *_args: Any) -> str:
        return report.replace("## Safety Flags", "M029 validation passed.\n\n## Safety Flags")

    assert "M029_POST_VALIDATION_FORBIDDEN_POSITIVE_CLAIM" in _codes(_errors(mutate))


def test_forbidden_claims_section_itself_is_allowed_for_dossier_inventory() -> None:
    assert _errors() == []


def test_missing_required_bounded_ref_diagnostic_fails_closed() -> None:
    def mutate(evidence: dict[str, Any], _report: str, diagnostic_rows: list[dict[str, Any]], *_args: Any) -> None:
        diagnostic_rows[:] = [row for row in diagnostic_rows if row["code"] != "M029_POST_VALIDATION_MISSING_BOUNDED_REF"]
        evidence["diagnostic_codes"] = sorted({row["code"] for row in diagnostic_rows})
        evidence["diagnostic_count"] = len(diagnostic_rows)
        return None

    assert "M029_POST_VALIDATION_DIAGNOSTIC_DRIFT" in _codes(_errors(mutate))


def test_malformed_jsonl_returns_nonzero_with_line_context(tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]) -> None:
    paths = _write_fixture_files(tmp_path)
    paths["diagnostics"].write_text('{"code":"ok"}\nnot-json\n', encoding="utf-8")
    monkeypatch.chdir(tmp_path)

    exit_code = main(_valid_argv(paths, tmp_path))

    assert exit_code == 2
    assert "diagnostics.jsonl:2" in capsys.readouterr().err


def test_absolute_input_path_is_rejected(tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]) -> None:
    paths = _write_fixture_files(tmp_path)
    monkeypatch.chdir(tmp_path)
    argv = _valid_argv(paths, tmp_path)
    argv[1] = str(paths["evidence"].resolve())

    exit_code = main(argv)

    assert exit_code == 2
    assert "repo-relative" in capsys.readouterr().err


def test_optional_replan_path_escape_is_rejected_as_diagnostic(tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]) -> None:
    paths = _write_fixture_files(tmp_path)
    monkeypatch.chdir(tmp_path)

    exit_code = main([*_valid_argv(paths, tmp_path), "--replan-evidence", "../outside/REPLAN.md"])

    assert exit_code == 1
    assert "M029_POST_VALIDATION_OPTIONAL_EVIDENCE_PATH_INVALID" in capsys.readouterr().err


def test_real_t01_artifacts_validate_in_repo_when_present() -> None:
    root = Path(__file__).parents[1]
    required = [
        root / "data/article_corpora/m029-unified-corpus-v1/post-validation-remediation/post-validation-remediation-evidence.json",
        root / "data/article_corpora/m029-unified-corpus-v1/post-validation-remediation/post-validation-remediation-report.md",
        root / "data/article_corpora/m029-unified-corpus-v1/post-validation-remediation/post-validation-remediation-diagnostics.jsonl",
        root / "data/article_corpora/m029-unified-corpus-v1/validation-remediation/remediation-evidence.json",
        root / "data/article_corpora/m029-unified-corpus-v1/validation-remediation/remediation-verify-summary.json",
        root / "data/article_corpora/m029-unified-corpus-v1/selection.json",
        root / "data/article_corpora/m029-unified-corpus-v1/readiness-verify-summary.json",
        root / "data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json",
    ]
    if not all(path.exists() for path in required):
        pytest.skip("M029 S08/T01 artifacts are not available in this checkout")

    evidence = json.loads(required[0].read_text(encoding="utf-8"))
    report = required[1].read_text(encoding="utf-8")
    diagnostic_rows = [json.loads(line) for line in required[2].read_text(encoding="utf-8").splitlines() if line.strip()]
    s07_evidence = json.loads(required[3].read_text(encoding="utf-8"))
    s07_summary = json.loads(required[4].read_text(encoding="utf-8"))
    m029_selection = json.loads(required[5].read_text(encoding="utf-8"))
    readiness = json.loads(required[6].read_text(encoding="utf-8"))
    m030_selection = json.loads(required[7].read_text(encoding="utf-8"))

    assert validate_remediation(evidence, report, diagnostic_rows, s07_evidence, s07_summary, m029_selection, readiness, m030_selection) == []
