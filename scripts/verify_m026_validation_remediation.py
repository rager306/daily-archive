#!/usr/bin/env python3
"""Validate the M026 S06 validation remediation class-audit package.

This verifier is intentionally read-only. It checks the S06 remediation audit
JSON, rendered markdown, S05 requirement-scope matrix, roadmap criteria source,
and current validation report for deterministic rerun-readiness without
executing artifact content or fetching external resources.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections.abc import Iterable
from pathlib import Path
from typing import Any

MILESTONE_ID = "M026-3rvvgp"
SLICE_ID = "S06"
SCHEMA_VERSION = "m026-validation-remediation-class-audit.v1"
MATRIX_SCHEMA_VERSION = "m026-requirement-scope-matrix.v1"
MATRIX_PATH = "doc/validation/m026_requirement_scope_matrix.json"
ROADMAP_PATH = ".gsd/milestones/M026-3rvvgp/M026-3rvvgp-ROADMAP.md"
VALIDATION_PATH = ".gsd/milestones/M026-3rvvgp/M026-3rvvgp-VALIDATION.md"
AUDIT_JSON_PATH = "doc/validation/m026_validation_remediation_class_audit.json"
AUDIT_MARKDOWN_PATH = "doc/validation/m026_validation_remediation_class_audit.md"

CANONICAL_CLASSES = ("Contract", "Integration", "Operational", "UAT")

BROAD_ACTIVE_OUT_OF_SCOPE_REQUIREMENTS = {
    "R019",
    "R022",
    "R023",
    "R024",
    "R027",
    "R029",
    "R031",
    "R032",
    "R033",
    "R035",
    "R051",
    "R052",
}
EXISTING_CONTEXT_REQUIREMENTS = {
    "R001",
    "R002",
    "R003",
    "R004",
    "R005",
    "R006",
    "R007",
    "R008",
    "R009",
    "R010",
    "R014",
    "R030",
}
REQUIRED_REQUIREMENT_IDS = (
    "R001",
    "R002",
    "R003",
    "R004",
    "R005",
    "R006",
    "R007",
    "R008",
    "R009",
    "R010",
    "R014",
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
)

REQUIRED_TOP_LEVEL_KEYS = {
    "schema_version",
    "milestone_id",
    "slice_id",
    "task_id",
    "generated_at_utc",
    "metadata_only",
    "source_inputs",
    "remediation_target",
    "criteria_source",
    "scope_matrix",
    "safety_flags",
    "requirement_coverage_interpretation",
    "canonical_verification_classes",
    "rerun_ready_validation_inputs",
    "safe_validation_wording",
    "forbidden_claims",
    "remaining_work",
    "quality_gates",
    "observability_impact",
}

REQUIRED_FALSE_SAFETY_FLAGS = {
    "loader_implementation_claimed",
    "batch_selection_implementation_claimed",
    "kg_import_or_readiness_claimed",
    "graph_validation_claimed",
    "import_ready_chunks_claimed",
    "scientific_kg_corpus_validation_claimed",
    "dspy_rlm_minimax_activation_claimed",
    "raw_payloads_embedded",
    "binary_payloads_embedded",
    "vector_payloads_embedded",
    "secrets_embedded",
    "production_ladybugdb_writes_claimed",
    "runtime_surface_changed",
    "loader_implementation_added",
    "graph_or_kg_import_authorized",
    "raw_article_text_embedded",
    "pdf_bytes_embedded",
    "production_connection_details_embedded",
}

EXPECTED_SPECIAL_REQUIREMENTS = {
    "R036": {
        "current_status": "active",
        "m026_applicability": "adjacent_evidence_not_full_requirement",
        "s05_verdict": "advanced_not_validated",
        "recommended_requirement_action": "remain_active",
    },
    "R040": {
        "current_status": "active",
        "m026_applicability": "in_scope_constraint_followed",
        "s05_verdict": "satisfied_for_m026_not_globally_validated",
        "recommended_requirement_action": "treat_as_followed_constraint_keep_active",
    },
    "R050": {
        "current_status": "active",
        "m026_applicability": "out_of_scope_future_consumer",
        "s05_verdict": "not_implemented_not_validated",
        "recommended_requirement_action": "remain_active",
    },
}

UNSAFE_POSITIVE_CLAIM_PHRASES = {
    "m026 implements the loader",
    "m026 implements loader",
    "m026 implements batch selection",
    "m026 authorizes kg import",
    "m026 authorizes kg import readiness",
    "m026 validates kg import readiness",
    "m026 validates graph readiness",
    "m026 validates scientific kg corpus behavior",
    "m026 validates import-ready chunks",
    "m026 validates import ready chunks",
    "m026 activates dspy",
    "m026 activates rlm",
    "m026 activates minimax",
    "m026 activates optimizer",
    "m026 writes to production ladybugdb",
    "m026 authorizes production ladybugdb writes",
    "m026 embeds raw article text",
    "m026 embeds raw pdfs",
    "m026 embeds pdf bytes",
    "m026 embeds binary payloads",
    "m026 embeds vector payloads",
    "m026 embeds secrets",
    "m026 fully validates r019",
    "m026 fully validates r022",
    "m026 fully validates r023",
    "m026 fully validates r024",
    "m026 fully validates r027",
    "m026 fully validates r029",
    "m026 fully validates r031",
    "m026 fully validates r032",
    "m026 fully validates r033",
    "m026 fully validates r035",
    "m026 fully validates r036",
    "m026 fully validates r051",
    "m026 fully validates r052",
    "m026 globally validates r040",
    "m026 implements r050",
    "m026 newly validates r001",
    "m026 newly validates r002",
    "m026 newly validates r003",
    "m026 newly validates r004",
    "m026 newly validates r005",
    "m026 newly validates r006",
    "m026 newly validates r007",
    "m026 newly validates r008",
    "m026 newly validates r009",
    "m026 newly validates r010",
    "m026 newly validates r014",
    "m026 newly validates r030",
    "m026 closes unrelated active requirements as validated",
}

EXPLICIT_FORBIDDEN_PATH_MARKERS = (
    ".forbidden_claims",
    ".must_not_claim",
    ".global_forbidden_claims",
)

ALLOWED_EVIDENCE_SUFFIXES = {".json", ".jsonl", ".md", ".txt", ".yaml", ".yml"}
PLANNING_CITATION_PREFIXES = (".gsd/", ".planning/", ".audits/")


class ValidationInputError(RuntimeError):
    """Raised when a required input cannot be read or parsed."""


def _json_path(parent: str, key: str | int) -> str:
    if isinstance(key, int):
        return f"{parent}[{key}]"
    return f"{parent}.{key}"


def _walk(value: Any, path: str = "$") -> Iterable[tuple[str, Any]]:
    yield path, value
    if isinstance(value, dict):
        for key, child in value.items():
            yield from _walk(child, _json_path(path, str(key)))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from _walk(child, _json_path(path, index))


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


def _load_json(path: Path, label: str) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise ValidationInputError(f"{label} file not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValidationInputError(f"malformed JSON in {label} at {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise ValidationInputError(f"{label} root must be an object: {path}")
    return payload


def _read_text(path: Path, label: str) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError as exc:
        raise ValidationInputError(f"{label} file not found: {path}") from exc


def _is_repo_relative_path(value: str) -> bool:
    path = Path(value)
    return (
        bool(value)
        and not path.is_absolute()
        and "://" not in value
        and "" not in path.parts
        and ".." not in path.parts
    )


def _validate_repo_relative_path(
    value: Any, path: str, *, require_supported_suffix: bool = True
) -> list[str]:
    if not isinstance(value, str):
        return [f"{path} must be a string repo-relative path"]
    if not value or value.strip() != value:
        return [f"{path} must not be blank or padded: {value!r}"]
    if not _is_repo_relative_path(value):
        return [
            f"{path} must be repo-relative and must not contain traversal or URL syntax: {value}"
        ]
    suffix = Path(value).suffix
    if require_supported_suffix and suffix and suffix not in ALLOWED_EVIDENCE_SUFFIXES:
        return [f"{path} has unsupported evidence suffix: {value}"]
    return []


def _validate_input_file_exists(path: Path, label: str) -> list[str]:
    if not path.exists():
        return [f"{label} input path does not exist: {path}"]
    if not path.is_file():
        return [f"{label} input path is not a file: {path}"]
    return []


def _rows_by_requirement(rows: Any, base_path: str) -> tuple[dict[str, dict[str, Any]], list[str]]:
    errors: list[str] = []
    if not isinstance(rows, list):
        return {}, [f"{base_path} must be a list"]
    by_id: dict[str, dict[str, Any]] = {}
    counts: dict[str, int] = {}
    for index, row in enumerate(rows):
        row_path = f"{base_path}[{index}]"
        if not isinstance(row, dict):
            errors.append(f"{row_path} must be an object")
            continue
        rid = row.get("requirement_id")
        if not isinstance(rid, str) or not rid:
            errors.append(f"{row_path}.requirement_id must be a non-empty string")
            continue
        by_id[rid] = row  # ty:ignore[invalid-assignment]
        counts[rid] = counts.get(rid, 0) + 1
    duplicates = sorted(rid for rid, count in counts.items() if count > 1)
    if duplicates:
        errors.append(f"{base_path} has duplicate requirement rows: {', '.join(duplicates)}")
    return by_id, errors


def _class_rows_by_name(rows: Any, base_path: str) -> tuple[dict[str, dict[str, Any]], list[str]]:
    errors: list[str] = []
    if not isinstance(rows, list):
        return {}, [f"{base_path} must be a list"]
    by_class: dict[str, dict[str, Any]] = {}
    counts: dict[str, int] = {}
    for index, row in enumerate(rows):
        row_path = f"{base_path}[{index}]"
        if not isinstance(row, dict):
            errors.append(f"{row_path} must be an object")
            continue
        class_name = row.get("class")
        if not isinstance(class_name, str) or not class_name:
            errors.append(f"{row_path}.class must be a non-empty string")
            continue
        by_class[class_name] = row  # ty:ignore[invalid-assignment]
        counts[class_name] = counts.get(class_name, 0) + 1
    duplicates = sorted(name for name, count in counts.items() if count > 1)
    if duplicates:
        errors.append(f"{base_path} has duplicate class rows: {', '.join(duplicates)}")
    return by_class, errors


def validate_top_level(audit: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    missing = sorted(REQUIRED_TOP_LEVEL_KEYS - set(audit))
    if missing:
        errors.append(f"$ missing top-level keys: {', '.join(missing)}")
    if audit.get("schema_version") != SCHEMA_VERSION:
        errors.append(f"$.schema_version must be {SCHEMA_VERSION}")
    if audit.get("milestone_id") != MILESTONE_ID:
        errors.append(f"$.milestone_id must be {MILESTONE_ID}")
    if audit.get("slice_id") != SLICE_ID:
        errors.append(f"$.slice_id must be {SLICE_ID}")
    if audit.get("metadata_only") is not True:
        errors.append("$.metadata_only must be true")

    for path, expected in (
        ("$.remediation_target.validation_report", VALIDATION_PATH),
        ("$.criteria_source.canonical_success_criteria_source", ROADMAP_PATH),
        ("$.scope_matrix.source", MATRIX_PATH),
        ("$.rerun_ready_validation_inputs.success_criteria_checklist_source", ROADMAP_PATH),
        ("$.rerun_ready_validation_inputs.requirement_coverage_source", MATRIX_PATH),
    ):
        value: Any = audit
        for part in path.removeprefix("$.").split("."):
            value = value.get(part) if isinstance(value, dict) else None
        if value != expected:
            errors.append(f"{path} must be {expected!r}, found {value!r}")
    return errors


def validate_safety_flags(audit: dict[str, Any]) -> list[str]:
    flags = audit.get("safety_flags")
    if not isinstance(flags, dict):
        return ["$.safety_flags must be an object"]
    errors: list[str] = []
    missing = sorted(REQUIRED_FALSE_SAFETY_FLAGS - set(flags))
    if missing:
        errors.append(f"$.safety_flags missing required flags: {', '.join(missing)}")
    for key in sorted(REQUIRED_FALSE_SAFETY_FLAGS & set(flags)):
        if flags.get(key) is not False:
            errors.append(f"$.safety_flags.{key} must be false")
    if flags.get("metadata_only") is not True:
        errors.append("$.safety_flags.metadata_only must be true")
    return errors


def validate_classes(audit: dict[str, Any], *, require_pass_classes: bool) -> list[str]:
    errors: list[str] = []
    class_rows, row_errors = _class_rows_by_name(
        audit.get("canonical_verification_classes"), "$.canonical_verification_classes"
    )
    errors.extend(row_errors)
    actual = set(class_rows)
    expected = set(CANONICAL_CLASSES)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        if missing:
            errors.append(
                f"$.canonical_verification_classes missing canonical classes: {', '.join(missing)}"
            )
        if extra:
            errors.append(
                f"$.canonical_verification_classes has unexpected classes: {', '.join(extra)}"
            )
    for class_name in CANONICAL_CLASSES:
        row = class_rows.get(class_name)
        if row is None:
            continue
        class_path = f"$.canonical_verification_classes[{class_name}]"
        if require_pass_classes and row.get("verdict") != "PASS":
            errors.append(f"{class_path}.verdict must be PASS under --require-pass-classes")
        if not isinstance(row.get("planned_check"), str) or not row.get("planned_check"):
            errors.append(f"{class_path}.planned_check must be a non-empty string")
        evidence_paths = row.get("evidence_paths")
        if not isinstance(evidence_paths, list) or not evidence_paths:
            errors.append(f"{class_path}.evidence_paths must be a non-empty list")
        else:
            for index, evidence in enumerate(evidence_paths):
                errors.extend(
                    _validate_repo_relative_path(evidence, f"{class_path}.evidence_paths[{index}]")
                )
        if not isinstance(row.get("safe_claim"), str) or not row.get("safe_claim"):
            errors.append(f"{class_path}.safe_claim must be a non-empty string")
        if not isinstance(row.get("must_not_claim"), list) or not row.get("must_not_claim"):
            errors.append(f"{class_path}.must_not_claim must be a non-empty list")

    rerun_rows, rerun_errors = _class_rows_by_name(
        audit.get("rerun_ready_validation_inputs", {}).get("verification_classes")
        if isinstance(audit.get("rerun_ready_validation_inputs"), dict)
        else None,
        "$.rerun_ready_validation_inputs.verification_classes",
    )
    errors.extend(rerun_errors)
    if set(rerun_rows) != expected:
        missing = sorted(expected - set(rerun_rows))
        extra = sorted(set(rerun_rows) - expected)
        if missing:
            errors.append(
                f"$.rerun_ready_validation_inputs.verification_classes missing classes: {', '.join(missing)}"
            )
        if extra:
            errors.append(
                f"$.rerun_ready_validation_inputs.verification_classes has unexpected classes: {', '.join(extra)}"
            )
    for class_name in CANONICAL_CLASSES:
        source = class_rows.get(class_name)
        rerun = rerun_rows.get(class_name)
        if source is None or rerun is None:
            continue
        rerun_path = f"$.rerun_ready_validation_inputs.verification_classes[{class_name}]"
        if rerun.get("planned_check") != source.get("planned_check"):
            errors.append(f"{rerun_path}.planned_check must match canonical class planned_check")
        if require_pass_classes and rerun.get("verdict") != "PASS":
            errors.append(f"{rerun_path}.verdict must be PASS under --require-pass-classes")
        evidence = rerun.get("evidence")
        if not isinstance(evidence, list) or not evidence:
            errors.append(f"{rerun_path}.evidence must be a non-empty list")
        else:
            if evidence != source.get("evidence_paths"):
                errors.append(f"{rerun_path}.evidence must match canonical class evidence_paths")
            for index, evidence_path in enumerate(evidence):
                errors.extend(
                    _validate_repo_relative_path(evidence_path, f"{rerun_path}.evidence[{index}]")
                )
    return errors


def validate_requirement_interpretation(audit: dict[str, Any], matrix: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if matrix.get("milestone_id") != MILESTONE_ID:
        errors.append(f"matrix $.milestone_id must be {MILESTONE_ID}")
    if matrix.get("schema_version") != MATRIX_SCHEMA_VERSION:
        errors.append(f"matrix $.schema_version must be {MATRIX_SCHEMA_VERSION}")
    if matrix.get("metadata_only") is not True:
        errors.append("matrix $.metadata_only must be true")

    declared_required = (
        audit.get("scope_matrix", {}).get("required_requirement_ids")
        if isinstance(audit.get("scope_matrix"), dict)
        else None
    )
    if set(declared_required or []) != set(REQUIRED_REQUIREMENT_IDS):
        errors.append(
            "$.scope_matrix.required_requirement_ids must exactly match the 27 S05 requirement IDs"
        )

    matrix_required = matrix.get("required_requirement_ids")
    if set(matrix_required or []) != set(REQUIRED_REQUIREMENT_IDS):
        errors.append(
            "matrix $.required_requirement_ids must exactly match the 27 S05 requirement IDs"
        )

    audit_rows, audit_row_errors = _rows_by_requirement(
        audit.get("requirement_coverage_interpretation", {}).get("requirement_rows")
        if isinstance(audit.get("requirement_coverage_interpretation"), dict)
        else None,
        "$.requirement_coverage_interpretation.requirement_rows",
    )
    matrix_rows, matrix_row_errors = _rows_by_requirement(
        matrix.get("requirements"), "matrix $.requirements"
    )
    errors.extend(audit_row_errors)
    errors.extend(matrix_row_errors)

    missing_audit = sorted(set(REQUIRED_REQUIREMENT_IDS) - set(audit_rows))
    missing_matrix = sorted(set(REQUIRED_REQUIREMENT_IDS) - set(matrix_rows))
    if missing_audit:
        errors.append(
            f"$.requirement_coverage_interpretation.requirement_rows missing ids: {', '.join(missing_audit)}"
        )
    if missing_matrix:
        errors.append(f"matrix $.requirements missing ids: {', '.join(missing_matrix)}")

    required_semantics = (
        audit.get("requirement_coverage_interpretation", {}).get("required_semantics")
        if isinstance(audit.get("requirement_coverage_interpretation"), dict)
        else None
    )
    if not isinstance(required_semantics, dict):
        errors.append("$.requirement_coverage_interpretation.required_semantics must be an object")
        required_semantics = {}

    for rid, expected in EXPECTED_SPECIAL_REQUIREMENTS.items():
        row = audit_rows.get(rid)
        matrix_row = matrix_rows.get(rid)
        for key, expected_value in expected.items():
            if row is not None and row.get(key) != expected_value:
                errors.append(
                    f"$.requirement_coverage_interpretation.requirement_rows[{rid}].{key} must be {expected_value!r}"
                )
            if matrix_row is not None and matrix_row.get(key) != expected_value:
                errors.append(f"matrix $.requirements[{rid}].{key} must be {expected_value!r}")
        semantics = required_semantics.get(rid)
        if not isinstance(semantics, dict):
            errors.append(
                f"$.requirement_coverage_interpretation.required_semantics.{rid} must be present"
            )
        elif semantics.get("classification") != expected["m026_applicability"]:
            errors.append(
                f"$.requirement_coverage_interpretation.required_semantics.{rid}.classification must be {expected['m026_applicability']!r}"
            )

    for rid in sorted(BROAD_ACTIVE_OUT_OF_SCOPE_REQUIREMENTS):
        row = audit_rows.get(rid)
        matrix_row = matrix_rows.get(rid)
        for label, candidate in (("audit", row), ("matrix", matrix_row)):
            if candidate is None:
                continue
            if candidate.get("current_status") != "active":
                errors.append(f"{label} {rid} must remain active")
            if candidate.get("m026_applicability") != "out_of_scope_active_requirement":
                errors.append(f"{label} {rid} must be classified out_of_scope_active_requirement")
            if candidate.get("recommended_requirement_action") != "remain_active":
                errors.append(f"{label} {rid} recommended action must be remain_active")

    for rid in sorted(EXISTING_CONTEXT_REQUIREMENTS):
        row = audit_rows.get(rid)
        matrix_row = matrix_rows.get(rid)
        for label, candidate in (("audit", row), ("matrix", matrix_row)):
            if candidate is None:
                continue
            if candidate.get("current_status") != "validated":
                errors.append(f"{label} {rid} must remain existing validated context")
            if (
                candidate.get("recommended_requirement_action")
                != "preserve_existing_validated_status"
            ):
                errors.append(
                    f"{label} {rid} recommended action must preserve existing validated status"
                )

    for rid in sorted(set(REQUIRED_REQUIREMENT_IDS) & set(audit_rows) & set(matrix_rows)):
        for key in (
            "current_status",
            "m026_applicability",
            "s05_verdict",
            "recommended_requirement_action",
        ):
            if audit_rows[rid].get(key) != matrix_rows[rid].get(key):
                errors.append(f"{rid} {key} mismatch between audit and matrix")

    broad_text = required_semantics.get("broad_active_requirements")
    if not isinstance(broad_text, str) or not all(
        rid in broad_text for rid in BROAD_ACTIVE_OUT_OF_SCOPE_REQUIREMENTS
    ):
        errors.append(
            "$.requirement_coverage_interpretation.required_semantics.broad_active_requirements must name all broad out-of-scope active requirements"
        )
    context_text = required_semantics.get("historical_validated_requirements")
    if not isinstance(context_text, str):
        errors.append(
            "$.requirement_coverage_interpretation.required_semantics.historical_validated_requirements must name all existing validated context requirements"
        )
    else:
        missing_context = [
            rid
            for rid in sorted(EXISTING_CONTEXT_REQUIREMENTS)
            if rid not in context_text
            and not (
                rid in {f"R00{number}" for number in range(1, 10)} | {"R010"}
                and "R001-R010" in context_text
            )
        ]
        if missing_context:
            errors.append(
                "$.requirement_coverage_interpretation.required_semantics.historical_validated_requirements "
                f"missing ids: {', '.join(missing_context)}"
            )
    return errors


def _strip_markdown_forbidden_section(markdown: str) -> str:
    return re.sub(r"(?ims)^## Forbidden Claims\n.*?(?=^## |\Z)", "\n", markdown)


def validate_unsafe_claims(audit: dict[str, Any], rendered_markdown: str) -> list[str]:
    errors: list[str] = []
    for path, value in _walk(audit):
        if any(marker in path for marker in EXPLICIT_FORBIDDEN_PATH_MARKERS):
            continue
        if isinstance(value, str):
            lowered = value.lower()
            for phrase in sorted(UNSAFE_POSITIVE_CLAIM_PHRASES):
                if phrase in lowered:
                    errors.append(f"{path} contains unsafe positive claim phrase: {phrase}")
    scan_markdown = _strip_markdown_forbidden_section(rendered_markdown).lower()
    for phrase in sorted(UNSAFE_POSITIVE_CLAIM_PHRASES):
        if phrase in scan_markdown:
            errors.append(
                f"markdown outside ## Forbidden Claims contains unsafe positive claim phrase: {phrase}"
            )
    return errors


def validate_rendered_markdown(audit: dict[str, Any], rendered_markdown: str) -> list[str]:
    errors: list[str] = []
    if not rendered_markdown.strip():
        return ["rendered markdown is empty"]
    required_markers = {
        "# M026 Validation Remediation Class Audit",
        AUDIT_JSON_PATH,
        ROADMAP_PATH,
        MATRIX_PATH,
        VALIDATION_PATH,
        "## Criteria Source",
        "## Requirement Coverage Interpretation",
        "## Canonical Verification Classes",
        "## Rerun-Ready Validation Inputs",
        "## Safe Validation Wording",
        "## Forbidden Claims",
        "## Safety Flags",
        "## Failure Modes",
        "## Load Profile",
        "## Negative Tests",
        "## Observability Impact",
        "scripts/verify_m026_requirement_scope_reconciliation.py",
    }
    commands = []
    rerun_inputs = audit.get("rerun_ready_validation_inputs")
    if isinstance(rerun_inputs, dict):
        commands = (
            rerun_inputs.get("commands") if isinstance(rerun_inputs.get("commands"), list) else []
        )
    required_markers.update(str(command) for command in commands if isinstance(command, str))

    for marker in sorted(required_markers):
        if marker not in rendered_markdown:
            errors.append(f"rendered markdown missing marker: {marker}")
    for rid in REQUIRED_REQUIREMENT_IDS:
        if rid not in rendered_markdown:
            errors.append(f"rendered markdown missing requirement id: {rid}")
    for term in (
        "out_of_scope_active_requirement",
        "adjacent_evidence_not_full_requirement",
        "in_scope_constraint_followed",
        "out_of_scope_future_consumer",
        "existing_validated_supporting_context",
    ):
        if term not in rendered_markdown:
            errors.append(f"rendered markdown missing classification term: {term}")
    for class_name in CANONICAL_CLASSES:
        if f"| {class_name} | PASS |" not in rendered_markdown:
            errors.append(f"rendered markdown missing PASS class row for {class_name}")
    flags = audit.get("safety_flags")
    if isinstance(flags, dict):
        for key, value in flags.items():
            marker = f"`{key}`: `{str(value).lower()}`"
            if marker not in rendered_markdown:
                errors.append(f"rendered markdown missing safety flag marker: {marker}")
    return errors


def _criterion_supported_by_roadmap(criterion: str, roadmap: str) -> bool:
    """Return True when a normalized criterion is supported by roadmap prose."""

    def normalize(text: str) -> str:
        text = text.lower().replace("loader's", "loader")
        text = re.sub(r"[^a-z0-9]+", " ", text)
        return re.sub(r"\s+", " ", text).strip()

    normalized_criterion = normalize(criterion)
    normalized_roadmap = normalize(roadmap)
    if normalized_criterion in normalized_roadmap:
        return True
    stopwords = {
        "a",
        "an",
        "and",
        "any",
        "are",
        "before",
        "does",
        "downstream",
        "exists",
        "exist",
        "for",
        "from",
        "get",
        "has",
        "have",
        "identifies",
        "implementation",
        "matches",
        "other",
        "path",
        "paths",
        "practical",
        "small",
        "source",
        "specified",
        "separately",
        "the",
        "with",
    }
    tokens = {
        token for token in normalized_criterion.split() if len(token) > 3 and token not in stopwords
    }
    return bool(tokens) and tokens.issubset(set(normalized_roadmap.split()))


def validate_source_texts(audit: dict[str, Any], roadmap: str, validation: str) -> list[str]:
    errors: list[str] = []
    criteria = audit.get("criteria_source")
    if not isinstance(criteria, dict):
        return ["$.criteria_source must be an object"]
    roadmap_criteria = criteria.get("roadmap_success_criteria")
    if not isinstance(roadmap_criteria, list) or not roadmap_criteria:
        errors.append("$.criteria_source.roadmap_success_criteria must be a non-empty list")
    else:
        for index, criterion in enumerate(roadmap_criteria):
            if not isinstance(criterion, str) or not criterion:
                errors.append(
                    f"$.criteria_source.roadmap_success_criteria[{index}] must be a non-empty string"
                )
            elif not _criterion_supported_by_roadmap(criterion, roadmap):
                errors.append(
                    f"$.criteria_source.roadmap_success_criteria[{index}] not found in roadmap: {criterion}"
                )
    if "needs-remediation" not in validation:
        errors.append(
            "validation report must still show needs-remediation for current remediation input"
        )
    for marker in ("Requirement Coverage", "Verification Class Compliance", "Remediation Plan"):
        if marker not in validation:
            errors.append(f"validation report missing section marker: {marker}")
    return errors


def validate_source_inputs(audit: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    inputs = audit.get("source_inputs")
    if not isinstance(inputs, list) or not all(isinstance(item, str) for item in inputs):
        return ["$.source_inputs must be a list of repo-relative path strings"]
    required = {MATRIX_PATH, ROADMAP_PATH, VALIDATION_PATH}
    missing_required = sorted(required - set(inputs))
    if missing_required:
        errors.append(
            f"$.source_inputs missing required source paths: {', '.join(missing_required)}"
        )
    for index, value in enumerate(inputs):
        errors.extend(_validate_repo_relative_path(value, f"$.source_inputs[{index}]"))
    return errors


def validate_audit(
    audit: dict[str, Any],
    rendered_markdown: str,
    matrix: dict[str, Any],
    roadmap: str,
    validation: str,
    *,
    require_pass_classes: bool,
    reject_unsafe_claims: bool,
) -> list[str]:
    errors: list[str] = []
    errors.extend(validate_top_level(audit))
    errors.extend(validate_source_inputs(audit))
    errors.extend(validate_safety_flags(audit))
    errors.extend(validate_classes(audit, require_pass_classes=require_pass_classes))
    errors.extend(validate_requirement_interpretation(audit, matrix))
    errors.extend(validate_rendered_markdown(audit, rendered_markdown))
    errors.extend(validate_source_texts(audit, roadmap, validation))
    if reject_unsafe_claims:
        errors.extend(validate_unsafe_claims(audit, rendered_markdown))
    return errors


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--audit",
        required=True,
        type=Path,
        help="Path to m026_validation_remediation_class_audit.json",
    )
    parser.add_argument(
        "--rendered", required=True, type=Path, help="Path to rendered audit markdown"
    )
    parser.add_argument(
        "--matrix", required=True, type=Path, help="Path to m026_requirement_scope_matrix.json"
    )
    parser.add_argument(
        "--roadmap", required=True, type=Path, help="Path to M026 roadmap criteria source"
    )
    parser.add_argument(
        "--validation", required=True, type=Path, help="Path to current M026 validation report"
    )
    parser.add_argument(
        "--require-pass-classes",
        action="store_true",
        help="Require all canonical class rows to have PASS verdicts",
    )
    parser.add_argument(
        "--reject-unsafe-claims",
        action="store_true",
        help="Reject unsafe positive closeout claims outside explicit forbidden-claims lists",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    input_errors: list[str] = []
    for label, path in (
        ("audit", args.audit),
        ("rendered markdown", args.rendered),
        ("matrix", args.matrix),
        ("roadmap", args.roadmap),
        ("validation", args.validation),
    ):
        input_errors.extend(_validate_input_file_exists(path, label))
    if input_errors:
        sys.stderr.write("M026 validation remediation input check failed:\n")
        for error in input_errors:
            sys.stderr.write(f"- {error}\n")
        return 2

    try:
        audit = _load_json(args.audit, "audit")
        matrix = _load_json(args.matrix, "matrix")
        rendered = _read_text(args.rendered, "rendered markdown")
        roadmap = _read_text(args.roadmap, "roadmap")
        validation = _read_text(args.validation, "validation")
    except ValidationInputError as exc:
        sys.stderr.write(f"ERROR: {exc}\n")
        return 2

    errors = validate_audit(
        audit,
        rendered,
        matrix,
        roadmap,
        validation,
        require_pass_classes=args.require_pass_classes,
        reject_unsafe_claims=args.reject_unsafe_claims,
    )
    if errors:
        sys.stderr.write("M026 validation remediation verification failed:\n")
        for error in errors:
            sys.stderr.write(f"- {error}\n")
        return 1

    class_count = len(audit.get("canonical_verification_classes", []))
    requirement_count = len(
        audit.get("requirement_coverage_interpretation", {}).get("requirement_rows", [])
    )
    sys.stdout.write(
        "M026 validation remediation verification passed: "
        f"{class_count} canonical classes and {requirement_count} requirement rows checked.\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
