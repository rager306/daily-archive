from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path
from typing import Any

from scripts import verify_m027_validation_remediation

CANONICAL_CLASSES = verify_m027_validation_remediation.CANONICAL_CLASSES
EXPECTED_CLASSIFICATIONS = verify_m027_validation_remediation.EXPECTED_CLASSIFICATIONS
MATRIX_PATH = verify_m027_validation_remediation.MATRIX_PATH
MATRIX_MARKDOWN_PATH = verify_m027_validation_remediation.MATRIX_MARKDOWN_PATH
ROADMAP_PATH = verify_m027_validation_remediation.ROADMAP_PATH
AUDIT_JSON_PATH = verify_m027_validation_remediation.AUDIT_JSON_PATH
S07_SUMMARY_PATH = verify_m027_validation_remediation.S07_SUMMARY_PATH
S07_REPORT_PATH = verify_m027_validation_remediation.S07_REPORT_PATH
S07_DIAGNOSTICS_PATH = verify_m027_validation_remediation.S07_DIAGNOSTICS_PATH
S06_SUMMARY_PATH = verify_m027_validation_remediation.S06_SUMMARY_PATH
REQUIRED_FALSE_SAFETY_FLAGS = verify_m027_validation_remediation.REQUIRED_FALSE_SAFETY_FLAGS
validate_audit = verify_m027_validation_remediation.validate_audit
main = verify_m027_validation_remediation.main


def _matrix_row(requirement_id: str) -> dict[str, Any]:
    expected = EXPECTED_CLASSIFICATIONS[requirement_id]
    return {
        "requirement_id": requirement_id,
        **expected,
        "evidence_paths": [MATRIX_PATH],
        "observed_m027_evidence": [f"{requirement_id} metadata-only evidence is preserved."],
        "allowed_claims": [f"{requirement_id} follows scoped M027 S08 interpretation."],
        "forbidden_claims": [f"Do not widen {requirement_id} beyond the scoped interpretation."],
        "remaining_work": [
            f"{requirement_id} remains governed by future direct evidence if needed."
        ],
        "rationale": f"{requirement_id} fixture rationale mirrors S08 semantics.",
    }


def _matrix() -> dict[str, Any]:
    rows = [_matrix_row(requirement_id) for requirement_id in sorted(EXPECTED_CLASSIFICATIONS)]
    return {
        "milestone_id": "M027-aakeky",
        "slice_id": "S08",
        "schema_version": "m027-requirement-scope-matrix.v1",
        "metadata_only": True,
        "safety_flags": {
            "metadata_only": True,
            **dict.fromkeys(REQUIRED_FALSE_SAFETY_FLAGS, False),
        },
        "required_requirement_ids": sorted(EXPECTED_CLASSIFICATIONS),
        "requirements": rows,
    }


def _class_row(class_name: str) -> dict[str, Any]:
    return {
        "class": class_name,
        "verdict": "PASS",
        "scope": f"{class_name} fixture scope is M027 S08 metadata-only remediation evidence.",
        "planned_check": f"{class_name} fixture planned check is local metadata-only validation.",
        "evidence_paths": [AUDIT_JSON_PATH, MATRIX_PATH],
        "safe_claim": f"{class_name} PASS is scoped to M027 S08 metadata-only remediation evidence.",
        "must_not_claim": [f"Do not overstate {class_name} evidence."],
    }


def _audit(matrix: dict[str, Any]) -> dict[str, Any]:
    class_rows = [_class_row(class_name) for class_name in CANONICAL_CLASSES]
    return {
        "schema_version": "m027-validation-remediation-class-audit.v1",
        "milestone_id": "M027-aakeky",
        "slice_id": "S08",
        "task_id": "T03",
        "generated_at_utc": "2026-06-02T00:00:00Z",
        "metadata_only": True,
        "source_inputs": [
            ROADMAP_PATH,
            MATRIX_PATH,
            MATRIX_MARKDOWN_PATH,
            S07_SUMMARY_PATH,
            S07_REPORT_PATH,
            S07_DIAGNOSTICS_PATH,
            S06_SUMMARY_PATH,
        ],
        "remediation_target": {
            "requirement_scope_matrix": MATRIX_PATH,
            "pipeline_readiness_synthesis_summary": S07_SUMMARY_PATH,
        },
        "criteria_source": {
            "canonical_success_criteria_source": ROADMAP_PATH,
            "criteria_source_decision": "Use roadmap fixture criteria.",
            "roadmap_success_criteria": [
                "The six user-supplied mixed-source article URLs are registered.",
                "R036-style provenance is advanced.",
                "The milestone remains preprocessing-only.",
            ],
        },
        "scope_matrix_reference": {
            "source": MATRIX_PATH,
            "markdown": MATRIX_MARKDOWN_PATH,
            "schema_version": "m027-requirement-scope-matrix.v1",
            "metadata_only": True,
            "required_requirement_ids": sorted(EXPECTED_CLASSIFICATIONS),
        },
        "safety_flags": {
            "metadata_only": True,
            **dict.fromkeys(REQUIRED_FALSE_SAFETY_FLAGS, False),
        },
        "requirement_coverage_interpretation": {
            "source_of_truth": MATRIX_PATH,
            "interpretation": "M027-advanced preprocessing evidence is not global validation.",
            "supported_claims": [
                "M027 S08 metadata-only remediation separates scoped evidence from future work."
            ],
            "requirement_rows": deepcopy(matrix["requirements"]),
        },
        "canonical_verification_classes": class_rows,
        "rerun_ready_validation_inputs": {
            "success_criteria_checklist_source": ROADMAP_PATH,
            "requirement_coverage_source": MATRIX_PATH,
            "class_audit_source": AUDIT_JSON_PATH,
            "verification_classes": [
                {
                    "class": row["class"],
                    "verdict": row["verdict"],
                    "planned_check": row["planned_check"],
                    "evidence": row["evidence_paths"],
                }
                for row in class_rows
            ],
            "commands": [
                "uv run python scripts/verify_m027_validation_remediation.py --validate-only"
            ],
        },
        "safe_validation_wording": [
            "M027 S08 supplies metadata-only validation remediation evidence."
        ],
        "forbidden_claims": ["M027 validates graph readiness"],
        "remaining_work": ["Future milestones supply direct validation evidence."],
        "quality_gates": {"failure_modes": [], "load_profile": [], "negative_tests": []},
        "observability_impact": "Diagnostics name exact JSON paths and class rows.",
    }


def _rendered(audit: dict[str, Any]) -> str:
    class_lines = "\n".join(
        f"| {row['class']} | {row['verdict']} | {row['scope']} | {row['planned_check']} |"
        for row in audit["canonical_verification_classes"]
    )
    requirement_lines = "\n".join(
        f"| {row['requirement_id']} | {row['current_status']} | `{row['m027_applicability']}` | `{row['s08_verdict']}` | `{row['recommended_requirement_action']}` |"
        for row in audit["requirement_coverage_interpretation"]["requirement_rows"]
    )
    return f"""# M027 Validation Remediation Class Audit

Source: `{AUDIT_JSON_PATH}`
Schema: `m027-validation-remediation-class-audit.v1`
metadata-only

## Requirement Coverage Interpretation
| Requirement | Status | Classification | Verdict | Action |
|---|---|---|---|---|
{requirement_lines}

## Canonical Verification Classes
| Class | Verdict | Scope | Planned check |
|---|---|---|---|
{class_lines}

## Rerun-Ready Validation Inputs
uv run python scripts/verify_m027_validation_remediation.py --validate-only

## Forbidden Claims
M027 validates graph readiness

## Failure Modes
fixture

## Load Profile
fixture

## Negative Tests
fixture

## Observability Impact
fixture
"""


def _matrix_markdown() -> str:
    return "# M027 Requirement Scope Matrix\nM027-advanced but not globally validated\nfuture/out-of-scope active requirements\nS07 closeout validation chain\n"


def _roadmap() -> str:
    return (
        "six user-supplied mixed-source article URLs\nR036-style provenance\npreprocessing-only\n"
    )


def _s07_summary() -> dict[str, Any]:
    return {
        "graph_import_allowed": False,
        "ladybugdb_written": False,
        "network_fetch_attempted": False,
    }


def _fixture() -> tuple[dict[str, Any], str, dict[str, Any], str, str, dict[str, Any], str]:
    matrix = _matrix()
    audit = _audit(matrix)
    return (
        audit,
        _rendered(audit),
        matrix,
        _matrix_markdown(),
        _roadmap(),
        _s07_summary(),
        "not_import_ready_validate_only",
    )


def _errors(mutator=None) -> list[str]:
    audit, rendered, matrix, matrix_markdown, roadmap, s07_summary, s07_report = _fixture()
    if mutator is not None:
        maybe_rendered = mutator(audit, rendered, matrix, s07_summary)
        if maybe_rendered is not None:
            rendered = maybe_rendered
    return validate_audit(
        audit, rendered, matrix, matrix_markdown, roadmap, s07_summary, s07_report
    )


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def test_positive_fixture_passes_without_project_files(tmp_path: Path) -> None:
    audit, rendered, matrix, matrix_markdown, roadmap, s07_summary, s07_report = _fixture()
    audit_path = tmp_path / "audit.json"
    rendered_path = tmp_path / "audit.md"
    matrix_path = tmp_path / "matrix.json"
    matrix_rendered_path = tmp_path / "matrix.md"
    roadmap_path = tmp_path / "roadmap.md"
    s07_summary_path = tmp_path / "s07.json"
    s07_report_path = tmp_path / "s07.md"
    _write_json(audit_path, audit)
    rendered_path.write_text(rendered, encoding="utf-8")
    _write_json(matrix_path, matrix)
    matrix_rendered_path.write_text(matrix_markdown, encoding="utf-8")
    roadmap_path.write_text(roadmap, encoding="utf-8")
    _write_json(s07_summary_path, s07_summary)
    s07_report_path.write_text(s07_report, encoding="utf-8")

    exit_code = main(
        [
            "--audit",
            str(audit_path),
            "--rendered",
            str(rendered_path),
            "--matrix",
            str(matrix_path),
            "--matrix-rendered",
            str(matrix_rendered_path),
            "--roadmap",
            str(roadmap_path),
            "--s07-summary",
            str(s07_summary_path),
            "--s07-report",
            str(s07_report_path),
        ]
    )

    assert exit_code == 0


def test_rejects_missing_canonical_class() -> None:
    def mutate(
        audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any], _summary: dict[str, Any]
    ) -> None:
        audit["canonical_verification_classes"] = [
            row for row in audit["canonical_verification_classes"] if row["class"] != "UAT"
        ]
        audit["rerun_ready_validation_inputs"]["verification_classes"] = [
            row
            for row in audit["rerun_ready_validation_inputs"]["verification_classes"]
            if row["class"] != "UAT"
        ]
        return None

    assert any("missing canonical classes: UAT" in error for error in _errors(mutate))


def test_rejects_extra_canonical_class() -> None:
    def mutate(
        audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any], _summary: dict[str, Any]
    ) -> None:
        extra = deepcopy(audit["canonical_verification_classes"][0])
        extra["class"] = "Performance"
        audit["canonical_verification_classes"].append(extra)
        audit["rerun_ready_validation_inputs"]["verification_classes"].append(
            {
                "class": "Performance",
                "verdict": "PASS",
                "planned_check": "Performance fixture planned check remains metadata-only.",
                "evidence": [AUDIT_JSON_PATH, MATRIX_PATH],
            }
        )
        return None

    errors = _errors(mutate)
    assert any("has unexpected classes: Performance" in error for error in errors)
    assert any(
        "verification_classes must list exactly the canonical classes" in error for error in errors
    )


def test_rejects_non_pass_class_verdict() -> None:
    def mutate(
        audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any], _summary: dict[str, Any]
    ) -> None:
        audit["canonical_verification_classes"][0]["verdict"] = "FAIL"
        audit["rerun_ready_validation_inputs"]["verification_classes"][0]["verdict"] = "FAIL"
        return None

    assert any("verdict must be PASS" in error for error in _errors(mutate))


def test_rejects_matrix_semantic_drift() -> None:
    def mutate(
        audit: dict[str, Any], _rendered: str, matrix: dict[str, Any], _summary: dict[str, Any]
    ) -> None:
        matrix["requirements"][0]["s08_verdict"] = "globally_validated"
        audit["requirement_coverage_interpretation"]["requirement_rows"][0]["s08_verdict"] = (
            "globally_validated"
        )
        return None

    assert any("s08_verdict must be" in error for error in _errors(mutate))


def test_rejects_semantic_drift_between_audit_and_matrix_rows() -> None:
    def mutate(
        audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any], _summary: dict[str, Any]
    ) -> None:
        audit["requirement_coverage_interpretation"]["requirement_rows"][0][
            "recommended_requirement_action"
        ] = "close_requirement"
        return None

    assert any(
        "audit R019 recommended_requirement_action must be remain_active" in error
        for error in _errors(mutate)
    )


def test_rejects_broad_active_requirement_reinterpretation_as_m027_deliverable() -> None:
    def mutate(
        audit: dict[str, Any], _rendered: str, matrix: dict[str, Any], _summary: dict[str, Any]
    ) -> None:
        row = next(row for row in matrix["requirements"] if row["requirement_id"] == "R031")
        row["m027_applicability"] = "m027_advanced_preprocessing_only"
        audit_row = next(
            row
            for row in audit["requirement_coverage_interpretation"]["requirement_rows"]
            if row["requirement_id"] == "R031"
        )
        audit_row["m027_applicability"] = "m027_advanced_preprocessing_only"
        return None

    assert any(
        "R031 m027_applicability must be future_out_of_scope_active_requirement" in error
        for error in _errors(mutate)
    )


def test_rejects_unsafe_safety_flag() -> None:
    def mutate(
        audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any], _summary: dict[str, Any]
    ) -> None:
        audit["safety_flags"]["graph_import_allowed"] = True
        return None

    assert any(
        "$.safety_flags.graph_import_allowed must be false" in error for error in _errors(mutate)
    )


def test_rejects_stale_markdown() -> None:
    def mutate(
        _audit: dict[str, Any], rendered: str, _matrix: dict[str, Any], _summary: dict[str, Any]
    ) -> str:
        return rendered.replace("| UAT | PASS |", "| UAT | FAIL |")

    assert any(
        "rendered markdown missing PASS class row for UAT" in error for error in _errors(mutate)
    )


def test_rejects_source_summary_claim_creep() -> None:
    def mutate(
        _audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any], summary: dict[str, Any]
    ) -> None:
        summary["graph_import_allowed"] = True
        return None

    assert any(
        "S07 summary graph_import_allowed must be false" in error for error in _errors(mutate)
    )


def test_rejects_missing_rerun_ready_validation_command() -> None:
    def mutate(
        audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any], _summary: dict[str, Any]
    ) -> None:
        audit["rerun_ready_validation_inputs"]["commands"] = [
            "uv run python scripts/verify_m027_requirement_scope_reconciliation.py --validate-only"
        ]
        return None

    assert any(
        "commands missing class-audit validate-only command" in error for error in _errors(mutate)
    )


def test_rejects_rerun_verification_class_evidence_missing_matrix() -> None:
    def mutate(
        audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any], _summary: dict[str, Any]
    ) -> None:
        audit["rerun_ready_validation_inputs"]["verification_classes"][0]["evidence"] = [
            AUDIT_JSON_PATH
        ]
        return None

    assert any(
        "rerun verification class Contract evidence must include matrix and audit JSON" in error
        for error in _errors(mutate)
    )


def test_rejects_missing_rerun_ready_validation_source_paths() -> None:
    def mutate(
        audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any], _summary: dict[str, Any]
    ) -> None:
        audit["rerun_ready_validation_inputs"]["requirement_coverage_source"] = (
            "doc/validation/other_matrix.json"
        )
        audit["rerun_ready_validation_inputs"]["class_audit_source"] = (
            "doc/validation/other_audit.json"
        )
        return None

    errors = _errors(mutate)
    assert any(
        "$.rerun_ready_validation_inputs.requirement_coverage_source must be" in error
        for error in errors
    )
    assert any(
        "$.rerun_ready_validation_inputs.class_audit_source must be" in error for error in errors
    )


def test_rejects_unsafe_positive_prose() -> None:
    def mutate(
        audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any], _summary: dict[str, Any]
    ) -> None:
        audit["safe_validation_wording"].append("M027 validates graph readiness for the corpus.")
        return None

    assert any(
        "contains unsafe positive claim phrase: m027 validates graph readiness" in error
        for error in _errors(mutate)
    )


def test_rejects_raw_binary_and_vector_payload_fields() -> None:
    def mutate(
        audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any], _summary: dict[str, Any]
    ) -> None:
        audit["raw_payload_fixture"] = "not allowed"
        audit["nested_payloads"] = {
            "binary_payload_fixture": "not allowed",
            "vector_payload_fixture": "not allowed",
        }
        return None

    errors = _errors(mutate)
    assert any(
        "$.raw_payload_fixture contains unsafe raw/binary/base64/vector/secret field name" in error
        for error in errors
    )
    assert any(
        "$.nested_payloads.binary_payload_fixture contains unsafe raw/binary/base64/vector/secret field name"
        in error
        for error in errors
    )
    assert any(
        "$.nested_payloads.vector_payload_fixture contains unsafe raw/binary/base64/vector/secret field name"
        in error
        for error in errors
    )


def test_rejects_raw_payload_leakage_marker() -> None:
    def mutate(
        audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any], _summary: dict[str, Any]
    ) -> None:
        audit["safe_validation_wording"].append("secret=value")
        return None

    assert any(
        "contains raw payload, base64, or secret leakage marker" in error
        for error in _errors(mutate)
    )
