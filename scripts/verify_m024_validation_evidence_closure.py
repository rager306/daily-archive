#!/usr/bin/env python3
"""Validate the M024 S09 validation evidence closure artifact.

This verifier is intentionally portable and fail-closed: it reads only the S08
requirement coverage matrix JSON and S09 closure JSON paths passed on the command
line. It does not inspect .gsd internals or dereference evidence paths embedded in
those artifacts.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

EXPECTED_MATRIX_SCHEMA = "m024-requirement-coverage-matrix.v1"
EXPECTED_CLOSURE_SCHEMA = "m024-validation-evidence-closure.v1"
EXPECTED_MILESTONE_ID = "M024-0xjwh9"
EXPECTED_MATRIX_SLICE_ID = "S08"
EXPECTED_CLOSURE_SLICE_ID = "S09"
RISK_GAP_ID = "S09-GAP-riskratchet-direct-evidence"
R036_GAP_ID = "S09-GAP-r036-requirement-update-decision"
PARTIAL_REQUIREMENTS = {"R024", "R027", "R029"}

REQUIRED_CLOSURE_TOP_LEVEL_KEYS = {
    "schema_version",
    "artifact_type",
    "milestone_id",
    "slice_id",
    "source_matrix",
    "closure_decisions",
    "requirement_treatments",
    "global_allowed_claims",
    "global_forbidden_claims",
}

UNSAFE_POSITIVE_CLAIM_PHRASES = {
    "authorizes kg import",
    "kg import authorization",
    "authorizes production ladybugdb writes",
    "authorizes production graph reads",
    "production writes are authorized",
    "positive graph readiness",
    "validates final graph readiness",
    "final graph readiness is validated",
    "kg validation to resume",
    "validates import-ready chunks",
    "positive kg import readiness",
    "promotes chunks to trusted kg facts",
    "fully validates r024",
    "r024 is fully validated",
    "fully validates r027",
    "r027 is fully validated",
    "fully validates r029",
    "r029 is fully validated",
    "validates 20-document graph quality",
    "validates one-week corpus graph quality",
    "validates 30-paper",
    "validates 100-paper",
    "canonical status parity is complete without db-backed requirement tooling",
    "r036 status parity is complete without db-backed requirement tooling",
    "manual status parity",
    "manually updated .gsd/requirements.md",
    "manually edited .gsd/requirements.md",
}


def _strings_from(value: Any) -> list[str]:
    if isinstance(value, str):
        return [value]
    if isinstance(value, list):
        strings: list[str] = []
        for item in value:
            strings.extend(_strings_from(item))
        return strings
    if isinstance(value, dict):
        strings: list[str] = []
        for item in value.values():
            strings.extend(_strings_from(item))
        return strings
    return []


def _rows_by_id(rows: Any, id_key: str, label: str, errors: list[str]) -> dict[str, dict[str, Any]]:
    if not isinstance(rows, list):
        errors.append(f"{label} must be a list")
        return {}
    by_id: dict[str, dict[str, Any]] = {}
    seen: dict[str, int] = {}
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            errors.append(f"{label}[{index}] must be an object")
            continue
        row_id = row.get(id_key)
        if not isinstance(row_id, str) or not row_id:
            errors.append(f"{label}[{index}] missing string {id_key}")
            continue
        seen[row_id] = seen.get(row_id, 0) + 1
        by_id[row_id] = row  # ty:ignore[invalid-assignment]
    for row_id, count in sorted(seen.items()):
        if count > 1:
            errors.append(f"duplicate {label} row for {row_id}")
    return by_id


def _list_contains(values: Any, needle: str) -> bool:
    if not isinstance(values, list):
        return False
    needle_lower = needle.lower()
    return any(isinstance(value, str) and needle_lower in value.lower() for value in values)


def _check_unsafe_positive_claims(
    closure: dict[str, Any],
    treatments: dict[str, dict[str, Any]],
    decisions: dict[str, dict[str, Any]],
    errors: list[str],
) -> None:
    positive_fields: list[tuple[str, Any]] = [
        ("global_allowed_claims", closure.get("global_allowed_claims")),
    ]
    for requirement_id, row in treatments.items():
        positive_fields.extend(
            [
                (f"{requirement_id}.allowed_claims", row.get("allowed_claims")),
                (
                    f"{requirement_id}.validation_rerun_position",
                    row.get("validation_rerun_position"),
                ),
                (f"{requirement_id}.m024_treatment", row.get("m024_treatment")),
            ]
        )
    for gap_id, row in decisions.items():
        positive_fields.append(
            (f"{gap_id}.allowed_validation_statement", row.get("allowed_validation_statement"))
        )

    for field_name, value in positive_fields:
        text = "\n".join(_strings_from(value)).lower()
        for phrase in sorted(UNSAFE_POSITIVE_CLAIM_PHRASES):
            if phrase in text:
                errors.append(f"{field_name} contains unsafe positive validation claim: {phrase}")


def validate_closure(
    matrix: dict[str, Any], closure: dict[str, Any], matrix_path: Path
) -> list[str]:
    errors: list[str] = []

    if matrix.get("schema_version") != EXPECTED_MATRIX_SCHEMA:
        errors.append(f"matrix schema_version must be {EXPECTED_MATRIX_SCHEMA}")
    if matrix.get("milestone_id") != EXPECTED_MILESTONE_ID:
        errors.append(f"matrix milestone_id must be {EXPECTED_MILESTONE_ID}")
    if matrix.get("slice_id") != EXPECTED_MATRIX_SLICE_ID:
        errors.append(f"matrix slice_id must be {EXPECTED_MATRIX_SLICE_ID}")

    missing_top = sorted(REQUIRED_CLOSURE_TOP_LEVEL_KEYS - set(closure))
    if missing_top:
        errors.append(f"closure missing top-level keys: {', '.join(missing_top)}")
    if closure.get("schema_version") != EXPECTED_CLOSURE_SCHEMA:
        errors.append(f"closure schema_version must be {EXPECTED_CLOSURE_SCHEMA}")
    if closure.get("artifact_type") != "validation_evidence_closure":
        errors.append("closure artifact_type must be validation_evidence_closure")
    if closure.get("milestone_id") != EXPECTED_MILESTONE_ID:
        errors.append(f"closure milestone_id must be {EXPECTED_MILESTONE_ID}")
    if closure.get("slice_id") != EXPECTED_CLOSURE_SLICE_ID:
        errors.append(f"closure slice_id must be {EXPECTED_CLOSURE_SLICE_ID}")

    source_matrix = closure.get("source_matrix")
    if not isinstance(source_matrix, dict):
        errors.append("closure source_matrix must be an object")
    else:
        source_json_path = source_matrix.get("json_path")
        if source_json_path != str(matrix_path):
            errors.append(
                f"closure source_matrix.json_path must match matrix argument path {matrix_path}"
            )
        if source_matrix.get("expected_schema_version") != EXPECTED_MATRIX_SCHEMA:
            errors.append(
                f"closure source_matrix.expected_schema_version must be {EXPECTED_MATRIX_SCHEMA}"
            )
        if not source_matrix.get("handoff_path"):
            errors.append("closure source_matrix.handoff_path must cite the S08 handoff")

    matrix_gaps = _rows_by_id(
        matrix.get("s09_handoff_gaps"), "gap_id", "matrix.s09_handoff_gaps", errors
    )
    risk_matrix_gap = matrix_gaps.get(RISK_GAP_ID)
    if not risk_matrix_gap:
        errors.append(f"matrix missing handoff gap {RISK_GAP_ID}")
    elif risk_matrix_gap.get("required_before_milestone_validation_rerun") is not True:
        errors.append(f"matrix {RISK_GAP_ID} must be required before validation rerun")
    if R036_GAP_ID not in matrix_gaps:
        errors.append(f"matrix missing handoff gap {R036_GAP_ID}")

    matrix_requirements = _rows_by_id(
        matrix.get("requirements"), "requirement_id", "matrix.requirements", errors
    )
    decisions = _rows_by_id(
        closure.get("closure_decisions"), "gap_id", "closure.closure_decisions", errors
    )
    treatments = _rows_by_id(
        closure.get("requirement_treatments"),
        "requirement_id",
        "closure.requirement_treatments",
        errors,
    )

    risk_decision = decisions.get(RISK_GAP_ID)
    if not risk_decision:
        errors.append(f"closure missing decision for {RISK_GAP_ID}")
    else:
        if (
            risk_decision.get("decision")
            != "closed_non_blocking_not_applicable_not_executed_for_m024"
        ):
            errors.append(
                f"{RISK_GAP_ID} decision must close as non-blocking/not-applicable/not-executed"
            )
        if risk_decision.get("blocking_for_m024_validation_rerun") is not False:
            errors.append(f"{RISK_GAP_ID} must not block the M024 validation rerun")
        if risk_decision.get("riskratchet_installed_or_required") is not False:
            errors.append(f"{RISK_GAP_ID} must not require or install riskratchet")
        if risk_decision.get("riskratchet_executed") is not False:
            errors.append(f"{RISK_GAP_ID} must not claim riskratchet execution")
        allowed_statement = risk_decision.get("allowed_validation_statement")
        if (
            not isinstance(allowed_statement, str)
            or "non-blocking" not in allowed_statement.lower()
            or "not applicable" not in allowed_statement.lower()
        ):
            errors.append(
                f"{RISK_GAP_ID} allowed_validation_statement must state non-blocking and not applicable treatment"
            )

    for requirement_id in sorted(PARTIAL_REQUIREMENTS):
        matrix_row = matrix_requirements.get(requirement_id)
        treatment_row = treatments.get(requirement_id)
        if not matrix_row:
            errors.append(f"matrix missing requirement row {requirement_id}")
        elif matrix_row.get("coverage_verdict") != "advanced_not_validated":
            errors.append(f"matrix {requirement_id} must remain advanced_not_validated")
        if not treatment_row:
            errors.append(f"closure missing treatment for {requirement_id}")
            continue
        if treatment_row.get("current_status_from_matrix") != "active":
            errors.append(f"closure {requirement_id} current_status_from_matrix must be active")
        if treatment_row.get("m024_treatment") != "advanced_not_validated":
            errors.append(f"closure {requirement_id} must use advanced_not_validated treatment")
        if treatment_row.get("validation_rerun_position") != "partial_advancement_only":
            errors.append(f"closure {requirement_id} must be partial_advancement_only")

    r030 = treatments.get("R030")
    if not r030:
        errors.append("closure missing treatment for R030")
    else:
        if r030.get("m024_treatment") != "covered_by_existing_s04_validation":
            errors.append("closure R030 must be covered_by_existing_s04_validation")
        if r030.get("validation_rerun_position") != "cite_existing_coverage_do_not_reopen":
            errors.append("closure R030 must cite existing coverage and not reopen")
        if (
            not _list_contains(r030.get("evidence_paths"), "S04")
            and "s04" not in "\n".join(_strings_from(r030.get("rationale"))).lower()
        ):
            errors.append("closure R030 must cite S04 coverage evidence")
    matrix_r030 = matrix_requirements.get("R030")
    if matrix_r030 and matrix_r030.get("coverage_verdict") != "covered_by_existing_validation":
        errors.append("matrix R030 must remain covered_by_existing_validation")

    r036 = treatments.get("R036")
    if not r036:
        errors.append("closure missing treatment for R036")
    else:
        if r036.get("m024_treatment") != "evidence_covered_status_parity_deferred":
            errors.append("closure R036 must be evidence_covered_status_parity_deferred")
        if (
            r036.get("validation_rerun_position")
            != "cite_implementation_coverage_without_claiming_canonical_status_parity"
        ):
            errors.append(
                "closure R036 must cite implementation coverage without claiming canonical status parity"
            )
        positive_r036_text = "\n".join(
            _strings_from(
                {
                    "allowed_claims": r036.get("allowed_claims"),
                    "validation_rerun_position": r036.get("validation_rerun_position"),
                    "m024_treatment": r036.get("m024_treatment"),
                }
            )
        ).lower()
        if (
            "db-backed" not in positive_r036_text
            and "gsd requirement tool" not in positive_r036_text
        ):
            errors.append(
                "closure R036 positive treatment must defer canonical parity to DB-backed requirement tooling"
            )
    matrix_r036 = matrix_requirements.get("R036")
    if matrix_r036 and matrix_r036.get("coverage_verdict") != "covered_by_existing_validation":
        errors.append("matrix R036 must remain covered_by_existing_validation for M024 evidence")

    if not isinstance(closure.get("global_allowed_claims"), list) or not closure.get(
        "global_allowed_claims"
    ):
        errors.append("closure global_allowed_claims must be a non-empty list")
    if not isinstance(closure.get("global_forbidden_claims"), list) or not closure.get(
        "global_forbidden_claims"
    ):
        errors.append("closure global_forbidden_claims must be a non-empty list")

    _check_unsafe_positive_claims(closure, treatments, decisions, errors)
    return errors


def _load_json(path: Path, label: str) -> tuple[dict[str, Any] | None, int]:
    try:
        value = json.loads(path.read_text())
    except FileNotFoundError:
        sys.stderr.write(f"ERROR: {label} file not found: {path}\n")
        return None, 2
    except json.JSONDecodeError as exc:
        sys.stderr.write(f"ERROR: malformed {label} JSON: {exc}\n")
        return None, 2
    if not isinstance(value, dict):
        sys.stderr.write(f"ERROR: {label} root must be an object\n")
        return None, 2
    return value, 0


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        sys.stderr.write(
            "usage: verify_m024_validation_evidence_closure.py <matrix.json> <closure.json>\n"
        )
        return 2

    matrix_path = Path(argv[1])
    closure_path = Path(argv[2])
    matrix, matrix_status = _load_json(matrix_path, "matrix")
    if matrix is None:
        return matrix_status
    closure, closure_status = _load_json(closure_path, "closure")
    if closure is None:
        return closure_status

    errors = validate_closure(matrix, closure, matrix_path)
    if errors:
        sys.stderr.write("M024 validation evidence closure validation failed:\n")
        for error in errors[:50]:
            sys.stderr.write(f"- {error}\n")
        if len(errors) > 50:
            sys.stderr.write(f"- ... {len(errors) - 50} additional errors omitted\n")
        return 1

    sys.stdout.write(
        "M024 validation evidence closure validation passed: "
        f"{len(closure['closure_decisions'])} closure decisions, "
        f"{len(closure['requirement_treatments'])} requirement treatments.\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
