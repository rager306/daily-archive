#!/usr/bin/env python3
"""Validate the M024 requirement applicability matrix.

This verifier is intentionally self-contained: it reads only the JSON matrix path
passed on the command line and does not inspect .gsd or any other repository file.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

REQUIRED_REQUIREMENT_IDS = {
    "R019",
    "R022",
    "R023",
    "R024",
    "R027",
    "R029",
    "R030",
    "R031",
    "R032",
    "R033",
    "R035",
    "R036",
    "R040",
    "R050",
    "R051",
    "R052",
}

IN_SCOPE_APPLICABILITIES = {
    "in_scope_advanced_partial",
    "in_scope_evidence_backed_candidate",
    "already_validated_covered_by_m024_s04",
}

REQUIRED_TOP_LEVEL_KEYS = {
    "milestone_id",
    "slice_id",
    "scope_boundary",
    "requirements",
    "s09_handoff_gaps",
    "review_notes",
}

UNSAFE_SCOPE_EXPANSION_PHRASES = {
    "m024 fully validates r024",
    "m024 validates 20-document graph quality",
    "m024 validates one-week corpus graph quality",
    "m024 authorizes kg import",
    "m024 validates positive graph readiness",
    "m024 authorizes kg validation to resume",
    "m024 validates import-ready chunks",
    "m024 validates positive kg import readiness",
    "m024 validates r050 artifact detection cli",
    "m024 validates minimax artifact detection integration",
    "m024 validates dspy prompt optimization readiness",
    "m024 validates 100-paper iterative automation",
    "m024 validates the 30-paper deviation scan",
}

PARTIAL_REQUIREMENTS = {"R024", "R027", "R029"}


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


def validate_matrix(matrix: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    missing_top = sorted(REQUIRED_TOP_LEVEL_KEYS - set(matrix))
    if missing_top:
        errors.append(f"missing top-level keys: {', '.join(missing_top)}")
    if matrix.get("milestone_id") != "M024-0xjwh9":
        errors.append("milestone_id must be M024-0xjwh9")
    if matrix.get("slice_id") != "S08":
        errors.append("slice_id must be S08")

    requirements = matrix.get("requirements")
    if not isinstance(requirements, list):
        return errors + ["requirements must be a list"]

    seen: dict[str, int] = {}
    rows: dict[str, dict[str, Any]] = {}
    for index, row in enumerate(requirements):
        if not isinstance(row, dict):
            errors.append(f"requirements[{index}] must be an object")
            continue
        requirement_id = row.get("requirement_id")
        if not isinstance(requirement_id, str):
            errors.append(f"requirements[{index}] missing string requirement_id")
            continue
        seen[requirement_id] = seen.get(requirement_id, 0) + 1
        rows[requirement_id] = row  # ty:ignore[invalid-assignment]
        for key in (
            "current_status",
            "m024_applicability",
            "coverage_verdict",
            "evidence_paths",
            "rationale",
            "allowed_claims",
            "forbidden_claims",
        ):
            if key not in row:
                errors.append(f"{requirement_id} missing {key}")

    missing_ids = sorted(REQUIRED_REQUIREMENT_IDS - set(seen))
    extra_ids = sorted(set(seen) - REQUIRED_REQUIREMENT_IDS)
    duplicate_ids = sorted(req_id for req_id, count in seen.items() if count > 1)
    if missing_ids:
        errors.append(f"missing requirement rows: {', '.join(missing_ids)}")
    if extra_ids:
        errors.append(f"unexpected requirement rows: {', '.join(extra_ids)}")
    if duplicate_ids:
        errors.append(f"duplicate requirement rows: {', '.join(duplicate_ids)}")

    for requirement_id, row in rows.items():
        evidence_paths = row.get("evidence_paths")
        if not isinstance(evidence_paths, list) or not all(
            isinstance(path, str) and path for path in evidence_paths
        ):
            errors.append(f"{requirement_id} evidence_paths must be a non-empty list of strings")
        if row.get("m024_applicability") in IN_SCOPE_APPLICABILITIES and not evidence_paths:
            errors.append(f"{requirement_id} is in-scope but has no evidence_paths")
        if not isinstance(row.get("allowed_claims"), list) or not row.get("allowed_claims"):
            errors.append(f"{requirement_id} allowed_claims must be non-empty")
        if not isinstance(row.get("forbidden_claims"), list) or not row.get("forbidden_claims"):
            errors.append(f"{requirement_id} forbidden_claims must be non-empty")

        positive_text = "\n".join(
            _strings_from(
                {
                    "coverage_verdict": row.get("coverage_verdict"),
                    "rationale": row.get("rationale"),
                    "allowed_claims": row.get("allowed_claims"),
                    "s09_followup": row.get("s09_followup"),
                }
            )
        ).lower()
        for phrase in UNSAFE_SCOPE_EXPANSION_PHRASES:
            if phrase in positive_text:
                errors.append(f"{requirement_id} contains unsafe scope-expansion claim: {phrase}")

    for requirement_id in PARTIAL_REQUIREMENTS:
        row = rows.get(requirement_id)
        if row and row.get("coverage_verdict") != "advanced_not_validated":
            errors.append(f"{requirement_id} must remain advanced_not_validated")
        if row and row.get("m024_applicability") != "in_scope_advanced_partial":
            errors.append(f"{requirement_id} must be classified in_scope_advanced_partial")
    if (
        rows.get("R030")
        and rows["R030"].get("coverage_verdict") != "covered_by_existing_validation"
    ):
        errors.append("R030 must be covered_by_existing_validation")
    if rows.get("R036") and rows["R036"].get("coverage_verdict") not in {
        "validated_pending_requirement_update_review",
        "covered_by_existing_validation",
    }:
        errors.append("R036 must be evidence-backed validated-pending-update or already covered")

    gaps = matrix.get("s09_handoff_gaps")
    if not isinstance(gaps, list) or not gaps:
        errors.append("s09_handoff_gaps must be a non-empty list")
    else:
        gap_text = "\n".join(_strings_from(gaps)).lower()
        if "riskratchet" not in gap_text:
            errors.append("missing S09 riskratchet handoff gap")
        if "r036" not in gap_text:
            errors.append("missing S09 R036 status/update handoff gap")
    return errors


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        sys.stderr.write("usage: verify_m024_requirement_coverage.py <matrix.json>\n")
        return 2
    matrix_path = Path(argv[1])
    try:
        matrix = json.loads(matrix_path.read_text())
    except FileNotFoundError:
        sys.stderr.write(f"ERROR: matrix file not found: {matrix_path}\n")
        return 2
    except json.JSONDecodeError as exc:
        sys.stderr.write(f"ERROR: malformed JSON: {exc}\n")
        return 2
    if not isinstance(matrix, dict):
        sys.stderr.write("ERROR: matrix root must be an object\n")
        return 2
    errors = validate_matrix(matrix)
    if errors:
        sys.stderr.write("M024 requirement coverage matrix validation failed:\n")
        for error in errors:
            sys.stderr.write(f"- {error}\n")
        return 1
    sys.stdout.write(
        "M024 requirement coverage matrix validation passed: "
        f"{len(matrix['requirements'])} requirement rows, "
        f"{len(matrix['s09_handoff_gaps'])} S09 handoff gaps.\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
