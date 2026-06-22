from __future__ import annotations

import importlib.util
import json
import sys
from collections.abc import Callable
from copy import deepcopy
from pathlib import Path
from typing import Any

import pytest

MODULE_PATH = Path(__file__).parents[1] / "scripts" / "verify_m026_validation_remediation.py"
spec = importlib.util.spec_from_file_location("verify_m026_validation_remediation", MODULE_PATH)
assert spec is not None and spec.loader is not None
verify_m026_validation_remediation = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = verify_m026_validation_remediation
spec.loader.exec_module(verify_m026_validation_remediation)

CANONICAL_CLASSES = verify_m026_validation_remediation.CANONICAL_CLASSES
REQUIRED_FALSE_SAFETY_FLAGS = verify_m026_validation_remediation.REQUIRED_FALSE_SAFETY_FLAGS
REQUIRED_REQUIREMENT_IDS = verify_m026_validation_remediation.REQUIRED_REQUIREMENT_IDS
ROADMAP_PATH = verify_m026_validation_remediation.ROADMAP_PATH
VALIDATION_PATH = verify_m026_validation_remediation.VALIDATION_PATH
MATRIX_PATH = verify_m026_validation_remediation.MATRIX_PATH
AUDIT_JSON_PATH = verify_m026_validation_remediation.AUDIT_JSON_PATH
BROAD_ACTIVE_OUT_OF_SCOPE_REQUIREMENTS = (
    verify_m026_validation_remediation.BROAD_ACTIVE_OUT_OF_SCOPE_REQUIREMENTS
)
EXISTING_CONTEXT_REQUIREMENTS = verify_m026_validation_remediation.EXISTING_CONTEXT_REQUIREMENTS
EXPECTED_SPECIAL_REQUIREMENTS = verify_m026_validation_remediation.EXPECTED_SPECIAL_REQUIREMENTS
validate_audit = verify_m026_validation_remediation.validate_audit
main = verify_m026_validation_remediation.main


def _base_matrix_row(requirement_id: str) -> dict[str, Any]:
    if requirement_id in EXPECTED_SPECIAL_REQUIREMENTS:
        semantics = EXPECTED_SPECIAL_REQUIREMENTS[requirement_id]
    elif requirement_id in BROAD_ACTIVE_OUT_OF_SCOPE_REQUIREMENTS:
        semantics = {
            "current_status": "active",
            "m026_applicability": "out_of_scope_active_requirement",
            "s05_verdict": "not_advanced_not_validated",
            "recommended_requirement_action": "remain_active",
        }
    elif requirement_id in EXISTING_CONTEXT_REQUIREMENTS:
        semantics = {
            "current_status": "validated",
            "m026_applicability": "existing_hermes_daily_archive_context",
            "s05_verdict": "existing_coverage_context_not_revalidated",
            "recommended_requirement_action": "preserve_existing_validated_status",
        }
        if requirement_id == "R014":
            semantics["m026_applicability"] = "existing_validated_compatibility_context"
            semantics["s05_verdict"] = "existing_coverage_supported_not_revalidated"
        if requirement_id == "R030":
            semantics["m026_applicability"] = "existing_validated_supporting_context"
            semantics["s05_verdict"] = "existing_coverage_supported_not_revalidated"
    else:  # pragma: no cover - protects future fixture drift if constants change.
        raise AssertionError(f"missing fixture semantics for {requirement_id}")

    return {
        "requirement_id": requirement_id,
        **semantics,
        "evidence_paths": ["doc/validation/m026_requirement_scope_matrix.json"],
        "observed_m026_evidence": [f"{requirement_id} metadata-only fixture evidence."],
        "allowed_claims": [
            f"{requirement_id} fixture row preserves the scoped M026 interpretation."
        ],
        "forbidden_claims": [
            f"Do not widen {requirement_id} beyond the scoped fixture interpretation."
        ],
        "remaining_work": [
            f"{requirement_id} remains governed by future direct evidence if needed."
        ],
        "rationale": f"{requirement_id} fixture rationale mirrors S05 semantics.",
    }


def _base_matrix() -> dict[str, Any]:
    return {
        "milestone_id": "M026-3rvvgp",
        "slice_id": "S05",
        "schema_version": "m026-requirement-scope-matrix.v1",
        "metadata_only": True,
        "required_requirement_ids": list(REQUIRED_REQUIREMENT_IDS),
        "requirements": [
            _base_matrix_row(requirement_id) for requirement_id in REQUIRED_REQUIREMENT_IDS
        ],
    }


def _class_row(class_name: str) -> dict[str, Any]:
    return {
        "class": class_name,
        "verdict": "PASS",
        "scope": f"{class_name} fixture scope remains metadata-only.",
        "planned_check": f"{class_name} fixture planned check is supported by metadata-only evidence.",
        "evidence_paths": ["doc/validation/m026_requirement_scope_matrix.json"],
        "safe_claim": f"{class_name} class passed against scoped M026 research and contract evidence.",
        "must_not_claim": [f"Do not overstate {class_name} evidence."],
    }


def _base_audit(matrix: dict[str, Any]) -> dict[str, Any]:
    class_rows = [_class_row(class_name) for class_name in CANONICAL_CLASSES]
    safety_flags = dict.fromkeys(REQUIRED_FALSE_SAFETY_FLAGS, False)
    safety_flags["metadata_only"] = True
    return {
        "schema_version": "m026-validation-remediation-class-audit.v1",
        "milestone_id": "M026-3rvvgp",
        "slice_id": "S06",
        "task_id": "T03",
        "generated_at_utc": "2026-06-01T00:00:00Z",
        "metadata_only": True,
        "source_inputs": [MATRIX_PATH, ROADMAP_PATH, VALIDATION_PATH],
        "remediation_target": {
            "validation_report": VALIDATION_PATH,
            "validation_verdict": "needs-remediation",
        },
        "criteria_source": {
            "canonical_success_criteria_source": ROADMAP_PATH,
            "criteria_source_decision": "Use roadmap fixture criteria.",
            "roadmap_success_criteria": [
                "source-capability matrix exists",
                "Hermes-agent digest is specified separately",
            ],
        },
        "scope_matrix": {
            "source": MATRIX_PATH,
            "schema_version": "m026-requirement-scope-matrix.v1",
            "metadata_only": True,
            "required_requirement_ids": list(REQUIRED_REQUIREMENT_IDS),
        },
        "safety_flags": safety_flags,
        "requirement_coverage_interpretation": {
            "source_of_truth": MATRIX_PATH,
            "required_semantics": {
                "R036": {"classification": "adjacent_evidence_not_full_requirement"},
                "R040": {"classification": "in_scope_constraint_followed"},
                "R050": {"classification": "out_of_scope_future_consumer"},
                "broad_active_requirements": ", ".join(
                    sorted(BROAD_ACTIVE_OUT_OF_SCOPE_REQUIREMENTS)
                ),
                "historical_validated_requirements": "R001-R010, R014, and R030",
            },
            "requirement_rows": deepcopy(matrix["requirements"]),
        },
        "canonical_verification_classes": class_rows,
        "rerun_ready_validation_inputs": {
            "success_criteria_checklist_source": ROADMAP_PATH,
            "requirement_coverage_source": MATRIX_PATH,
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
                "uv run python scripts/verify_m026_validation_remediation.py --require-pass-classes --reject-unsafe-claims"
            ],
        },
        "safe_validation_wording": ["M026 remains a metadata-only validation remediation fixture."],
        "forbidden_claims": ["M026 implements the loader"],
        "remaining_work": ["Future milestones supply direct runtime evidence."],
        "quality_gates": {
            "Q5": "fixture failure modes",
            "Q6": "fixture load profile",
            "Q7": "fixture negative tests",
        },
        "observability_impact": "Diagnostics name exact JSON paths, classes, requirements, and stale Markdown markers.",
    }


def _render_markdown(audit: dict[str, Any]) -> str:
    flag_lines = "\n".join(
        f"- `{key}`: `{str(value).lower()}`" for key, value in audit["safety_flags"].items()
    )
    class_lines = "\n".join(
        f"| {row['class']} | {row['verdict']} | {row['scope']} | {row['planned_check']} | {', '.join(row['evidence_paths'])} |"
        for row in audit["canonical_verification_classes"]
    )
    requirement_lines = "\n".join(
        f"| {row['requirement_id']} | {row['current_status']} | `{row['m026_applicability']}` | `{row['s05_verdict']}` | fixture |"
        for row in audit["requirement_coverage_interpretation"]["requirement_rows"]
    )
    command_lines = "\n".join(audit["rerun_ready_validation_inputs"]["commands"])
    return f"""# M026 Validation Remediation Class Audit

Source: `{AUDIT_JSON_PATH}`

## Criteria Source
- {ROADMAP_PATH}
- {MATRIX_PATH}
- {VALIDATION_PATH}

## Requirement Coverage Interpretation
| Requirement | Status | Classification | S05 verdict | Notes |
|---|---|---|---|---|
{requirement_lines}

## Canonical Verification Classes
| Class | Verdict | Scope | Planned check | Evidence paths |
|---|---|---|---|---|
{class_lines}

## Rerun-Ready Validation Inputs
{command_lines}

## Safe Validation Wording
M026 remains scoped to research, contracts, and metadata-only evidence.

## Forbidden Claims
- M026 implements the loader
- M026 fully validates R036
- M026 globally validates R040
- M026 implements R050

## Safety Flags
{flag_lines}

## Failure Modes
Malformed JSON, missing files, and stale Markdown bubble as verifier diagnostics.

## Load Profile
Static fixture validation is bounded by fixture size and deterministic iteration.

## Negative Tests
Negative pytest cases mutate one field per unsafe drift class.

## Observability Impact
Diagnostics name exact JSON paths, classes, requirements, and stale Markdown markers.

scripts/verify_m026_requirement_scope_reconciliation.py
out_of_scope_active_requirement
adjacent_evidence_not_full_requirement
in_scope_constraint_followed
out_of_scope_future_consumer
existing_validated_supporting_context
"""


def _roadmap() -> str:
    return """
# M026 Roadmap Fixture
- source-capability matrix exists for arXiv API fixture evidence.
- Hermes-agent digest is specified separately from loader raw evidence bundle.
"""


def _validation() -> str:
    return """
# M026 Validation Fixture
verdict: needs-remediation
## Requirement Coverage
## Verification Class Compliance
## Remediation Plan
"""


def _fixture() -> tuple[dict[str, Any], str, dict[str, Any], str, str]:
    matrix = _base_matrix()
    audit = _base_audit(matrix)
    return audit, _render_markdown(audit), matrix, _roadmap(), _validation()


def _errors(
    mutate: Callable[[dict[str, Any], str, dict[str, Any]], str | None] | None = None,
) -> list[str]:
    audit, rendered, matrix, roadmap, validation = _fixture()
    if mutate is not None:
        maybe_rendered = mutate(audit, rendered, matrix)
        if maybe_rendered is not None:
            rendered = maybe_rendered
    return validate_audit(
        audit,
        rendered,
        matrix,
        roadmap,
        validation,
        require_pass_classes=True,
        reject_unsafe_claims=True,
    )


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")


def test_positive_fixture_passes_without_reading_project_gsd(tmp_path: Path) -> None:
    audit, rendered, matrix, roadmap, validation = _fixture()
    audit_path = tmp_path / "audit.json"
    rendered_path = tmp_path / "audit.md"
    matrix_path = tmp_path / "matrix.json"
    roadmap_path = tmp_path / "roadmap.md"
    validation_path = tmp_path / "validation.md"
    _write_json(audit_path, audit)
    rendered_path.write_text(rendered, encoding="utf-8")
    _write_json(matrix_path, matrix)
    roadmap_path.write_text(roadmap, encoding="utf-8")
    validation_path.write_text(validation, encoding="utf-8")

    exit_code = main(
        [
            "--audit",
            str(audit_path),
            "--rendered",
            str(rendered_path),
            "--matrix",
            str(matrix_path),
            "--roadmap",
            str(roadmap_path),
            "--validation",
            str(validation_path),
            "--require-pass-classes",
            "--reject-unsafe-claims",
        ]
    )

    assert exit_code == 0


@pytest.mark.parametrize(
    ("class_name", "expected"),
    [
        ("UAT", "missing canonical classes: UAT"),
        ("Extra", "has unexpected classes: Extra"),
    ],
)
def test_rejects_missing_or_extra_class(class_name: str, expected: str) -> None:
    def mutate(audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any]) -> None:
        if class_name == "UAT":
            audit["canonical_verification_classes"] = [
                row for row in audit["canonical_verification_classes"] if row["class"] != class_name
            ]
            audit["rerun_ready_validation_inputs"]["verification_classes"] = [
                row
                for row in audit["rerun_ready_validation_inputs"]["verification_classes"]
                if row["class"] != class_name
            ]
        else:
            audit["canonical_verification_classes"].append(_class_row(class_name))
            audit["rerun_ready_validation_inputs"]["verification_classes"].append(
                {
                    "class": class_name,
                    "verdict": "PASS",
                    "planned_check": "Extra fixture planned check.",
                    "evidence": ["doc/validation/m026_requirement_scope_matrix.json"],
                }
            )
        return None

    assert any(expected in error for error in _errors(mutate))


def test_rejects_non_pass_class_under_strict_mode() -> None:
    def mutate(audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any]) -> None:
        audit["canonical_verification_classes"][0]["verdict"] = "FAIL"
        audit["rerun_ready_validation_inputs"]["verification_classes"][0]["verdict"] = "FAIL"
        return None

    errors = _errors(mutate)

    assert any("verdict must be PASS under --require-pass-classes" in error for error in errors)


def test_rejects_wrong_criteria_source() -> None:
    def mutate(audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any]) -> None:
        audit["criteria_source"]["canonical_success_criteria_source"] = (
            "doc/validation/not-roadmap.md"
        )
        return None

    assert any(
        "$.criteria_source.canonical_success_criteria_source" in error for error in _errors(mutate)
    )


def test_rejects_stale_markdown() -> None:
    def mutate(_audit: dict[str, Any], rendered: str, _matrix: dict[str, Any]) -> str:
        return rendered.replace("| UAT | PASS |", "| UAT | FAIL |")

    assert any(
        "rendered markdown missing PASS class row for UAT" in error for error in _errors(mutate)
    )


@pytest.mark.parametrize(
    ("requirement_id", "field", "value", "expected"),
    [
        ("R036", "s05_verdict", "validated", "R036"),
        ("R040", "s05_verdict", "globally_validated", "R040"),
        ("R050", "s05_verdict", "implemented_and_validated", "R050"),
        ("R019", "current_status", "validated", "audit R019 must remain active"),
        (
            "R022",
            "m026_applicability",
            "missing_touched_coverage",
            "audit R022 must be classified out_of_scope_active_requirement",
        ),
    ],
)
def test_rejects_requirement_semantic_drift(
    requirement_id: str, field: str, value: str, expected: str
) -> None:
    def mutate(audit: dict[str, Any], _rendered: str, matrix: dict[str, Any]) -> None:
        for rows in (
            audit["requirement_coverage_interpretation"]["requirement_rows"],
            matrix["requirements"],
        ):
            next(row for row in rows if row["requirement_id"] == requirement_id)[field] = value
        return None

    assert any(expected in error for error in _errors(mutate))


@pytest.mark.parametrize(
    "flag", ["kg_import_or_readiness_claimed", "raw_payloads_embedded", "binary_payloads_embedded"]
)
def test_rejects_unsafe_true_safety_flag(flag: str) -> None:
    def mutate(audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any]) -> None:
        audit["safety_flags"][flag] = True
        return None

    assert any(f"$.safety_flags.{flag} must be false" in error for error in _errors(mutate))


@pytest.mark.parametrize(
    ("key", "expected"),
    [
        ("raw_article_text_embedded", "$.safety_flags.raw_article_text_embedded must be false"),
        ("pdf_bytes_embedded", "$.safety_flags.pdf_bytes_embedded must be false"),
    ],
)
def test_rejects_raw_or_binary_payload_field_drift(key: str, expected: str) -> None:
    def mutate(audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any]) -> None:
        audit["safety_flags"][key] = True
        return None

    assert any(expected in error for error in _errors(mutate))


def test_rejects_unsafe_positive_phrase_outside_forbidden_claims() -> None:
    def mutate(audit: dict[str, Any], _rendered: str, _matrix: dict[str, Any]) -> None:
        audit["safe_validation_wording"].append("M026 implements the loader.")
        return None

    assert any(
        "contains unsafe positive claim phrase: m026 implements the loader" in error
        for error in _errors(mutate)
    )


def test_rejects_unsafe_positive_phrase_in_markdown_outside_forbidden_claims() -> None:
    def mutate(_audit: dict[str, Any], rendered: str, _matrix: dict[str, Any]) -> str:
        return rendered.replace(
            "M026 remains scoped to research, contracts, and metadata-only evidence.",
            "M026 validates graph readiness.",
        )

    assert any(
        "markdown outside ## Forbidden Claims contains unsafe positive claim phrase" in error
        for error in _errors(mutate)
    )
