#!/usr/bin/env python3
"""Validate the M028 requirement scope reconciliation matrix.

The verifier treats the S07 matrix as executable validation-remediation evidence.
It checks that M028-covered smoke-loader requirements are not promoted to global
validation, broad active Scientific KG requirements remain future/out-of-scope,
R036 remains an already-validated provenance requirement only supported by M028,
R040 remains an active safety constraint, evidence paths are safe and
repo-relative, safety flags fail closed, raw payload markers are absent, and the
fresh-reader markdown twin stays in sync with the JSON source of truth.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

MILESTONE_ID = "M028-8hwqjk"
SLICE_ID = "S07"
SCHEMA_VERSION = "m028-requirement-scope-matrix.v1"
DEFAULT_MATRIX = Path("doc/validation/m028_requirement_scope_matrix.json")
DEFAULT_RENDERED = Path("doc/validation/m028_requirement_scope_matrix.md")

M028_SMOKE_LOADER_REQUIREMENT_IDS = {"R024", "R027", "R029"}
FUTURE_OUT_OF_SCOPE_REQUIREMENT_IDS = {
    "R019",
    "R022",
    "R023",
    "R031",
    "R032",
    "R033",
    "R035",
    "R050",
    "R051",
    "R052",
}
R036_SUPPORTED_REQUIREMENT_IDS = {"R036"}
R040_SAFETY_REQUIREMENT_IDS = {"R040"}
REQUIRED_REQUIREMENT_IDS = (
    M028_SMOKE_LOADER_REQUIREMENT_IDS
    | FUTURE_OUT_OF_SCOPE_REQUIREMENT_IDS
    | R036_SUPPORTED_REQUIREMENT_IDS
    | R040_SAFETY_REQUIREMENT_IDS
)

EXPECTED_CLASSIFICATIONS: dict[str, dict[str, str]] = {
    **{
        rid: {
            "current_status": "active",
            "m028_applicability": "m028_smoke_loader_evidence_only",
            "s07_verdict": "advanced_by_smoke_loader_not_globally_validated",
            "recommended_requirement_action": "remain_active_with_m028_evidence_note",
        }
        for rid in sorted(M028_SMOKE_LOADER_REQUIREMENT_IDS)
    },
    **{
        rid: {
            "current_status": "active",
            "m028_applicability": "future_out_of_scope_active_requirement",
            "s07_verdict": "not_advanced_not_validated_by_m028",
            "recommended_requirement_action": "remain_active",
        }
        for rid in sorted(FUTURE_OUT_OF_SCOPE_REQUIREMENT_IDS)
    },
    "R036": {
        "current_status": "validated",
        "m028_applicability": "already_validated_requirement_supported_by_replay_provenance",
        "s07_verdict": "supported_not_revalidated_by_m028",
        "recommended_requirement_action": "preserve_existing_validated_status",
    },
    "R040": {
        "current_status": "active",
        "m028_applicability": "active_safety_constraint_preserved_and_advanced",
        "s07_verdict": "preserved_and_advanced_not_validated",
        "recommended_requirement_action": "remain_active_with_m028_safety_evidence_note",
    },
}

REQUIRED_TOP_LEVEL_KEYS = {
    "milestone_id",
    "slice_id",
    "schema_version",
    "generated_by_task",
    "metadata_only",
    "no_runtime_surface_changed",
    "safety_flags",
    "source_input_paths",
    "scope_boundary",
    "global_forbidden_claims",
    "required_requirement_ids",
    "requirements",
    "validation_recommendation",
    "verifier_expectations",
    "quality_gates",
}

REQUIRED_ROW_KEYS = {
    "requirement_id",
    "current_status",
    "m028_applicability",
    "s07_verdict",
    "recommended_requirement_action",
    "evidence_paths",
    "observed_m028_evidence",
    "allowed_claims",
    "forbidden_claims",
    "remaining_work",
    "rationale",
}

REQUIRED_SAFETY_FLAGS = {
    "metadata_only",
    "network_fetch_attempted",
    "runtime_surface_changed",
    "parser_or_chunker_changed",
    "kg_import_or_readiness_claimed",
    "graph_validation_claimed",
    "import_ready_chunks_claimed",
    "trusted_fact_promotion_claimed",
    "scientific_kg_corpus_validation_claimed",
    "dspy_rlm_minimax_activation_claimed",
    "unattended_scaling_claimed",
    "raw_payloads_embedded",
    "binary_payloads_embedded",
    "base64_payloads_embedded",
    "vector_payloads_embedded",
    "secrets_embedded",
    "production_ladybugdb_writes_claimed",
    "production_write_attempted",
}

FALSE_SAFETY_FLAGS = REQUIRED_SAFETY_FLAGS - {"metadata_only"}

UNSAFE_TRUE_BOOLEAN_KEYS = {
    "network_fetch_attempted",
    "runtime_surface_changed",
    "parser_or_chunker_changed",
    "parser_changed",
    "chunker_changed",
    "kg_import_or_readiness_claimed",
    "graph_validation_claimed",
    "import_ready_chunks_claimed",
    "trusted_fact_promotion_claimed",
    "scientific_kg_corpus_validation_claimed",
    "dspy_rlm_minimax_activation_claimed",
    "unattended_scaling_claimed",
    "raw_payloads_embedded",
    "binary_payloads_embedded",
    "base64_payloads_embedded",
    "vector_payloads_embedded",
    "secrets_embedded",
    "production_ladybugdb_writes_claimed",
    "production_write_attempted",
    "graph_import_allowed",
    "trusted_kg_import_allowed",
    "kg_import_ready",
    "import_ready",
    "import_eligible",
    "production_import_attempted",
    "production_ladybugdb_write_allowed",
    "ladybugdb_written",
    "dspy_activated",
    "rlm_activated",
    "minimax_activated",
    "optimizer_activated",
    "raw_article_text_included",
    "raw_payloads_included",
    "binary_payloads_included",
    "base64_payloads_included",
    "vectors_included",
    "secrets_included",
}

UNSAFE_FIELD_NAME_FRAGMENTS = {
    "raw_article_text",
    "raw_text",
    "raw_payload",
    "binary_payload",
    "base64_payload",
    "vector_payload",
    "embedding_payload",
    "secret_value",
    "production_connection",
}

UNSAFE_POSITIVE_CLAIM_PHRASES = {
    "m028 globally validates r024",
    "m028 globally validates r027",
    "m028 globally validates r029",
    "m028 globally validates r036",
    "m028 globally validates r040",
    "m028 newly validates r036 globally",
    "m028 closes all future provenance validation obligations",
    "m028 fully validates r019",
    "m028 fully validates r022",
    "m028 fully validates r023",
    "m028 fully validates r024",
    "m028 fully validates r027",
    "m028 fully validates r029",
    "m028 fully validates r031",
    "m028 fully validates r032",
    "m028 fully validates r033",
    "m028 fully validates r035",
    "m028 fully validates r036",
    "m028 fully validates r040",
    "m028 fully validates r050",
    "m028 fully validates r051",
    "m028 fully validates r052",
    "m028 advances r019 as a deliverable",
    "m028 advances r022 as a deliverable",
    "m028 advances r023 as a deliverable",
    "m028 advances r031 as a deliverable",
    "m028 advances r032 as a deliverable",
    "m028 advances r033 as a deliverable",
    "m028 advances r035 as a deliverable",
    "m028 advances r050 as a deliverable",
    "m028 advances r051 as a deliverable",
    "m028 advances r052 as a deliverable",
    "m028 changes r035 to validated",
    "m028 delivers validation-batch quota top-up",
    "m028 materializes deterministic replacement candidates",
    "m028 delivers source-ready replacement candidates",
    "m028 validates graph readiness",
    "m028 authorizes kg import",
    "m028 authorizes kg import readiness",
    "m028 validates kg import readiness",
    "m028 validates scientific kg corpus",
    "m028 validates import-ready chunks",
    "m028 validates import ready chunks",
    "m028 promotes trusted facts",
    "m028 writes to production ladybugdb",
    "m028 authorizes production ladybugdb writes",
    "m028 authorizes production writes",
    "m028 changes parser or chunker behavior",
    "m028 enables new infrastructure in the main scientific kg process",
    "m028 activates dspy",
    "m028 activates rlm",
    "m028 activates minimax",
    "m028 activates optimizer",
    "m028 proves unattended scaling",
    "m028 embeds raw article text",
    "m028 embeds raw pdfs",
    "m028 embeds pdf bytes",
    "m028 embeds binary payloads",
    "m028 embeds base64 payloads",
    "m028 embeds vector payloads",
    "m028 embeds secrets",
}

ALLOWED_EVIDENCE_SUFFIXES = {".json", ".jsonl", ".md", ".txt", ".yaml", ".yml"}
PLANNING_PATH_PREFIXES = (".gsd/", ".planning/", ".audits/")
POSITIVE_TEXT_KEYS = {
    "allowed_claims",
    "observed_m028_evidence",
    "rationale",
    "remaining_work",
    "validation_recommendation",
    "review_notes",
}


class MatrixValidationError(RuntimeError):
    """Raised for verifier misuse or unreadable inputs rather than matrix failures."""


def _diag(code: str, path: str, message: str) -> str:
    return f"{code}: {path}: {message}"


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


def _walk(value: Any, path: str = "$") -> list[tuple[str, Any]]:
    items = [(path, value)]
    if isinstance(value, dict):
        for key, child in value.items():
            items.extend(_walk(child, f"{path}.{key}"))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            items.extend(_walk(child, f"{path}[{index}]"))
    return items


def _is_planning_path(path: str) -> bool:
    return path.replace("\\", "/").startswith(PLANNING_PATH_PREFIXES)


def _field_name_from_path(path: str) -> str:
    return path.rsplit(".", maxsplit=1)[-1].split("[", maxsplit=1)[0].lower()


def _validate_evidence_path(
    *,
    owner_path: str,
    path_value: str,
    repo_root: Path,
    require_planning_evidence: bool,
) -> list[str]:
    errors: list[str] = []
    if not path_value or path_value.strip() != path_value:
        return [
            _diag(
                "M028_MATRIX_EVIDENCE_PATH_INVALID",
                owner_path,
                f"evidence path is blank or padded: {path_value!r}",
            )
        ]
    if "://" in path_value:
        errors.append(
            _diag(
                "M028_MATRIX_EVIDENCE_PATH_INVALID",
                owner_path,
                f"evidence path must be a repo-relative path, not a URL: {path_value}",
            )
        )
    path = Path(path_value)
    if path.is_absolute():
        errors.append(
            _diag(
                "M028_MATRIX_EVIDENCE_PATH_INVALID",
                owner_path,
                f"evidence path must be relative: {path_value}",
            )
        )
        return errors
    if any(part == ".." for part in path.parts):
        errors.append(
            _diag(
                "M028_MATRIX_EVIDENCE_PATH_INVALID",
                owner_path,
                f"evidence path must not escape the repo: {path_value}",
            )
        )
        return errors
    if path.suffix and path.suffix not in ALLOWED_EVIDENCE_SUFFIXES:
        errors.append(
            _diag(
                "M028_MATRIX_EVIDENCE_PATH_INVALID",
                owner_path,
                f"evidence path has unsupported extension: {path_value}",
            )
        )
    if _is_planning_path(path_value) and not require_planning_evidence:
        return errors
    resolved = (repo_root / path).resolve()
    try:
        resolved.relative_to(repo_root.resolve())
    except ValueError:
        errors.append(
            _diag(
                "M028_MATRIX_EVIDENCE_PATH_INVALID",
                owner_path,
                f"evidence path escapes repo root: {path_value}",
            )
        )
        return errors
    if not resolved.exists():
        errors.append(
            _diag(
                "M028_MATRIX_EVIDENCE_PATH_INVALID",
                owner_path,
                f"evidence path does not exist: {path_value}",
            )
        )
    return errors


def _validate_safety_flags(matrix: dict[str, Any]) -> list[str]:
    safety_flags = matrix.get("safety_flags")
    if not isinstance(safety_flags, dict):
        return [_diag("M028_MATRIX_UNSAFE_FLAG_TRUE", "$.safety_flags", "must be an object")]
    errors: list[str] = []
    missing = sorted(REQUIRED_SAFETY_FLAGS - set(safety_flags))
    if missing:
        errors.append(
            _diag(
                "M028_MATRIX_UNSAFE_FLAG_TRUE",
                "$.safety_flags",
                f"missing required fields: {', '.join(missing)}",
            )
        )
    if safety_flags.get("metadata_only") is not True:
        errors.append(
            _diag("M028_MATRIX_UNSAFE_FLAG_TRUE", "$.safety_flags.metadata_only", "must be true")
        )
    for key in sorted(FALSE_SAFETY_FLAGS & set(safety_flags)):
        if safety_flags.get(key) is not False:
            errors.append(
                _diag("M028_MATRIX_UNSAFE_FLAG_TRUE", f"$.safety_flags.{key}", "must be false")
            )
    return errors


def _positive_text_for_matrix(matrix: dict[str, Any]) -> dict[str, str]:
    positive: dict[str, str] = {}
    scope = matrix.get("scope_boundary")
    if isinstance(scope, dict):
        for key in ("summary", "in_scope", "safe_next_step"):
            positive[f"$.scope_boundary.{key}"] = "\n".join(_strings_from(scope.get(key)))
    positive["$.validation_recommendation"] = "\n".join(
        _strings_from(matrix.get("validation_recommendation"))
    )
    quality_gates = matrix.get("quality_gates")
    if isinstance(quality_gates, dict):
        positive["$.quality_gates.observability_impact"] = "\n".join(
            _strings_from(quality_gates.get("observability_impact"))
        )
    requirements = matrix.get("requirements")
    if isinstance(requirements, list):
        for index, row in enumerate(requirements):
            if not isinstance(row, dict):
                continue
            rid = (
                row.get("requirement_id")
                if isinstance(row.get("requirement_id"), str)
                else f"index {index}"
            )
            for key in POSITIVE_TEXT_KEYS:
                if key in row:
                    positive[f"$.requirements[{index}]({rid}).{key}"] = "\n".join(
                        _strings_from(row.get(key))
                    )
    return positive


def validate_matrix(
    matrix: dict[str, Any],
    rendered_markdown: str,
    *,
    repo_root: Path | None = None,
    required_requirements: set[str] | None = None,
    reject_unsafe_claims: bool = True,
    require_planning_evidence: bool = False,
) -> list[str]:
    """Return validation diagnostics for the M028 matrix and rendered markdown."""

    repo_root = repo_root or Path.cwd()
    required = required_requirements or REQUIRED_REQUIREMENT_IDS
    errors: list[str] = []

    missing_top = sorted(REQUIRED_TOP_LEVEL_KEYS - set(matrix))
    if missing_top:
        errors.append(
            _diag(
                "M028_MATRIX_MARKDOWN_STALE",
                "$",
                f"missing top-level keys: {', '.join(missing_top)}",
            )
        )
    if matrix.get("milestone_id") != MILESTONE_ID:
        errors.append(
            _diag("M028_MATRIX_MARKDOWN_STALE", "$.milestone_id", f"must be {MILESTONE_ID}")
        )
    if matrix.get("slice_id") != SLICE_ID:
        errors.append(_diag("M028_MATRIX_MARKDOWN_STALE", "$.slice_id", f"must be {SLICE_ID}"))
    if matrix.get("schema_version") != SCHEMA_VERSION:
        errors.append(
            _diag("M028_MATRIX_MARKDOWN_STALE", "$.schema_version", f"must be {SCHEMA_VERSION}")
        )
    if matrix.get("metadata_only") is not True:
        errors.append(_diag("M028_MATRIX_UNSAFE_FLAG_TRUE", "$.metadata_only", "must be true"))
    if matrix.get("no_runtime_surface_changed") is not True:
        errors.append(
            _diag("M028_MATRIX_UNSAFE_FLAG_TRUE", "$.no_runtime_surface_changed", "must be true")
        )
    errors.extend(_validate_safety_flags(matrix))

    source_paths = matrix.get("source_input_paths")
    if (
        not isinstance(source_paths, list)
        or not source_paths
        or not all(isinstance(item, str) and item for item in source_paths)
    ):
        errors.append(
            _diag(
                "M028_MATRIX_EVIDENCE_PATH_INVALID",
                "$.source_input_paths",
                "must be a non-empty list of strings",
            )
        )
    elif not any(
        path.startswith(
            "data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/"
        )
        for path in source_paths
    ):
        errors.append(
            _diag(
                "M028_MATRIX_EVIDENCE_PATH_INVALID",
                "$.source_input_paths",
                "must include M028 smoke replay closeout artifacts",
            )
        )
    else:
        for index, path_value in enumerate(source_paths):
            errors.extend(
                _validate_evidence_path(
                    owner_path=f"$.source_input_paths[{index}]",
                    path_value=path_value,
                    repo_root=repo_root,
                    require_planning_evidence=require_planning_evidence,
                )
            )

    declared_required = matrix.get("required_requirement_ids")
    if not isinstance(declared_required, list) or not all(
        isinstance(item, str) for item in declared_required
    ):
        errors.append(
            _diag(
                "M028_MATRIX_REQUIRED_ROW_MISSING",
                "$.required_requirement_ids",
                "must be a list of strings",
            )
        )
    elif set(declared_required) != required:
        missing_declared = sorted(required - set(declared_required))
        extra_declared = sorted(set(declared_required) - required)
        if missing_declared:
            errors.append(
                _diag(
                    "M028_MATRIX_REQUIRED_ROW_MISSING",
                    "$.required_requirement_ids",
                    f"missing ids: {', '.join(missing_declared)}",
                )
            )
        if extra_declared:
            errors.append(
                _diag(
                    "M028_MATRIX_REQUIRED_ROW_MISSING",
                    "$.required_requirement_ids",
                    f"has unexpected ids: {', '.join(extra_declared)}",
                )
            )

    requirements = matrix.get("requirements")
    if not isinstance(requirements, list):
        return errors + [
            _diag("M028_MATRIX_REQUIRED_ROW_MISSING", "$.requirements", "must be a list")
        ]

    seen: dict[str, int] = {}
    rows: dict[str, dict[str, Any]] = {}
    row_paths: dict[str, str] = {}
    for index, row in enumerate(requirements):
        row_path = f"$.requirements[{index}]"
        if not isinstance(row, dict):
            errors.append(_diag("M028_MATRIX_REQUIRED_ROW_MISSING", row_path, "must be an object"))
            continue
        rid = row.get("requirement_id")
        if not isinstance(rid, str) or not rid:
            errors.append(
                _diag(
                    "M028_MATRIX_REQUIRED_ROW_MISSING",
                    f"{row_path}.requirement_id",
                    "missing or not a string",
                )
            )
            continue
        seen[rid] = seen.get(rid, 0) + 1
        rows[rid] = row
        row_paths[rid] = row_path
        missing_row_keys = sorted(REQUIRED_ROW_KEYS - set(row))
        if missing_row_keys:
            errors.append(
                _diag(
                    "M028_MATRIX_REQUIRED_ROW_MISSING",
                    row_path,
                    f"{rid} missing required fields: {', '.join(missing_row_keys)}",
                )
            )

    missing_ids = sorted(required - set(seen))
    extra_ids = sorted(set(seen) - required)
    duplicate_ids = sorted(rid for rid, count in seen.items() if count > 1)
    if missing_ids:
        errors.append(
            _diag(
                "M028_MATRIX_REQUIRED_ROW_MISSING",
                "$.requirements",
                f"missing requirement rows: {', '.join(missing_ids)}",
            )
        )
    if extra_ids:
        errors.append(
            _diag(
                "M028_MATRIX_REQUIRED_ROW_MISSING",
                "$.requirements",
                f"unexpected requirement rows: {', '.join(extra_ids)}",
            )
        )
    if duplicate_ids:
        errors.append(
            _diag(
                "M028_MATRIX_REQUIRED_ROW_DUPLICATE",
                "$.requirements",
                f"duplicate requirement rows: {', '.join(duplicate_ids)}",
            )
        )

    for rid, expected in EXPECTED_CLASSIFICATIONS.items():
        row = rows.get(rid)
        if row is None:
            continue
        row_path = row_paths[rid]
        for key, expected_value in expected.items():
            if row.get(key) != expected_value:
                errors.append(
                    _diag(
                        "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                        f"{row_path}.{key}",
                        f"{rid} {key} must be {expected_value}, found {row.get(key)!r}",
                    )
                )

    for rid, row in rows.items():
        row_path = row_paths[rid]
        evidence_paths = row.get("evidence_paths")
        if not isinstance(evidence_paths, list) or not evidence_paths:
            errors.append(
                _diag(
                    "M028_MATRIX_EVIDENCE_PATH_INVALID",
                    f"{row_path}.evidence_paths",
                    f"{rid} evidence_paths must be a non-empty list",
                )
            )
        else:
            for index, path_value in enumerate(evidence_paths):
                path_location = f"{row_path}.evidence_paths[{index}]"
                if not isinstance(path_value, str):
                    errors.append(
                        _diag(
                            "M028_MATRIX_EVIDENCE_PATH_INVALID",
                            path_location,
                            f"{rid} evidence path must be a string: {path_value!r}",
                        )
                    )
                    continue
                errors.extend(
                    _validate_evidence_path(
                        owner_path=path_location,
                        path_value=path_value,
                        repo_root=repo_root,
                        require_planning_evidence=require_planning_evidence,
                    )
                )
        for key in (
            "allowed_claims",
            "forbidden_claims",
            "remaining_work",
            "observed_m028_evidence",
        ):
            value = row.get(key)
            if (
                not isinstance(value, list)
                or not value
                or not all(isinstance(item, str) and item for item in value)
            ):
                errors.append(
                    _diag(
                        "M028_MATRIX_REQUIRED_ROW_MISSING",
                        f"{row_path}.{key}",
                        f"{rid} {key} must be a non-empty list of strings",
                    )
                )
        if not isinstance(row.get("rationale"), str) or not row.get("rationale"):
            errors.append(
                _diag(
                    "M028_MATRIX_REQUIRED_ROW_MISSING",
                    f"{row_path}.rationale",
                    f"{rid} rationale must be a non-empty string",
                )
            )

    for rid in sorted(M028_SMOKE_LOADER_REQUIREMENT_IDS):
        row = rows.get(rid)
        if row is None:
            continue
        row_path = row_paths[rid]
        positive = "\n".join(
            _strings_from({key: row.get(key) for key in POSITIVE_TEXT_KEYS})
        ).lower()
        if row.get("current_status") != "active":
            errors.append(
                _diag(
                    "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                    f"{row_path}.current_status",
                    f"{rid} current_status must remain active",
                )
            )
        if row.get("s07_verdict") != "advanced_by_smoke_loader_not_globally_validated":
            errors.append(
                _diag(
                    "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                    f"{row_path}.s07_verdict",
                    f"{rid} must be smoke-loader evidence only, not globally validated",
                )
            )
        if (
            "fully validates" in positive
            or "global validation accepted" in positive
            or "global validation closed" in positive
        ):
            errors.append(
                _diag(
                    "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                    row_path,
                    f"{rid} must be M028 smoke-loader evidence, not globally validated",
                )
            )

    for rid in sorted(FUTURE_OUT_OF_SCOPE_REQUIREMENT_IDS):
        row = rows.get(rid)
        if row is None:
            continue
        row_path = row_paths[rid]
        positive = "\n".join(
            _strings_from({key: row.get(key) for key in POSITIVE_TEXT_KEYS})
        ).lower()
        if (
            row.get("current_status") != "active"
            or row.get("s07_verdict") != "not_advanced_not_validated_by_m028"
        ):
            errors.append(
                _diag(
                    "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                    row_path,
                    f"{rid} must remain active/future/out-of-scope and not validated by M028",
                )
            )
        if (
            "advanced" in positive
            and "not advanced" not in positive
            and "future/out-of-scope" not in positive
        ):
            errors.append(
                _diag(
                    "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                    row_path,
                    f"{rid} must not be described as advanced by M028",
                )
            )

    row = rows.get("R036")
    if row is not None:
        row_path = row_paths["R036"]
        positive = "\n".join(
            _strings_from({key: row.get(key) for key in POSITIVE_TEXT_KEYS})
        ).lower()
        if row.get("current_status") != "validated":
            errors.append(
                _diag(
                    "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                    f"{row_path}.current_status",
                    "R036 must preserve its already validated status",
                )
            )
        if row.get("s07_verdict") != "supported_not_revalidated_by_m028":
            errors.append(
                _diag(
                    "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                    f"{row_path}.s07_verdict",
                    "R036 must be supported but not revalidated by M028",
                )
            )
        if (
            "already validated" not in positive
            or "support" not in positive
            or "provenance" not in positive
        ):
            errors.append(
                _diag(
                    "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                    row_path,
                    "R036 must be already validated and only supported by M028 replay provenance",
                )
            )
        if (
            "newly validates" in positive
            or "globally validates" in positive
            or "closes all future" in positive
        ):
            errors.append(
                _diag(
                    "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                    row_path,
                    "R036 must not carry a false global validation claim",
                )
            )

    row = rows.get("R040")
    if row is not None:
        row_path = row_paths["R040"]
        positive = "\n".join(
            _strings_from({key: row.get(key) for key in POSITIVE_TEXT_KEYS})
        ).lower()
        if row.get("current_status") != "active":
            errors.append(
                _diag(
                    "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                    f"{row_path}.current_status",
                    "R040 must remain an active safety constraint",
                )
            )
        if row.get("s07_verdict") != "preserved_and_advanced_not_validated":
            errors.append(
                _diag(
                    "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                    f"{row_path}.s07_verdict",
                    "R040 must be preserved/advanced but not validated",
                )
            )
        if (
            "active safety constraint" not in positive
            or "not claiming" not in positive
            and "without claiming" not in positive
        ):
            errors.append(
                _diag(
                    "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                    row_path,
                    "R040 must remain an active safety constraint without main Scientific KG activation claims",
                )
            )
        if (
            "fully validates" in positive
            or "production writes" in positive
            and "without claiming" not in positive
        ):
            errors.append(
                _diag(
                    "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                    row_path,
                    "R040 must not carry a false global validation or production-write claim",
                )
            )

    if reject_unsafe_claims:
        for path, text in _positive_text_for_matrix(matrix).items():
            lowered = text.lower()
            for phrase in sorted(UNSAFE_POSITIVE_CLAIM_PHRASES):
                if phrase in lowered:
                    errors.append(
                        _diag(
                            "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                            path,
                            f"contains unsafe claim phrase: {phrase}",
                        )
                    )

    for path, value in _walk(matrix):
        field_name = _field_name_from_path(path)
        if isinstance(value, bool) and value is True and field_name in UNSAFE_TRUE_BOOLEAN_KEYS:
            errors.append(
                _diag("M028_MATRIX_UNSAFE_FLAG_TRUE", path, "unsafe boolean field must not be true")
            )
        if any(fragment in field_name for fragment in UNSAFE_FIELD_NAME_FRAGMENTS):
            if not path.startswith("$.safety_flags."):
                errors.append(
                    _diag(
                        "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                        path,
                        "contains unsafe raw/binary/base64/vector/secret field name",
                    )
                )
        if isinstance(value, str):
            lowered = value.lower()
            if (
                "-----begin" in lowered
                or "base64," in lowered
                or "secret=" in lowered
                or "password=" in lowered
            ):
                errors.append(
                    _diag(
                        "M028_MATRIX_UNSAFE_CLAIM_LEAKED",
                        path,
                        "contains raw payload, base64, or secret leakage marker",
                    )
                )

    if not rendered_markdown.strip():
        errors.append(
            _diag("M028_MATRIX_MARKDOWN_STALE", "$rendered", "rendered markdown is empty")
        )
    for marker in (
        "# M028 Requirement Scope Matrix",
        "doc/validation/m028_requirement_scope_matrix.json",
        SCHEMA_VERSION,
        "metadata-only",
        "bounded universal-loader runtime smoke",
        "future/out-of-scope active requirements",
        "R036 was already validated",
        "R040 was preserved and advanced as an active safety constraint",
        "## Failure Modes",
        "## Load Profile",
        "## Negative Tests",
        "## Observability Impact",
    ):
        if marker not in rendered_markdown:
            errors.append(
                _diag(
                    "M028_MATRIX_MARKDOWN_STALE",
                    "$rendered",
                    f"rendered markdown missing marker: {marker}",
                )
            )
    for rid in sorted(required):
        if rid not in rendered_markdown:
            errors.append(
                _diag(
                    "M028_MATRIX_MARKDOWN_STALE",
                    "$rendered",
                    f"rendered markdown missing requirement id: {rid}",
                )
            )
        expected = EXPECTED_CLASSIFICATIONS.get(rid, {})
        for key, expected_value in expected.items():
            if expected_value not in rendered_markdown:
                errors.append(
                    _diag(
                        "M028_MATRIX_MARKDOWN_STALE",
                        "$rendered",
                        f"rendered markdown missing {rid} {key}: {expected_value}",
                    )
                )

    return errors


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise MatrixValidationError(
            _diag("M028_MATRIX_JSON_MALFORMED", "$", f"matrix file not found: {path}")
        ) from exc
    except json.JSONDecodeError as exc:
        raise MatrixValidationError(
            _diag("M028_MATRIX_JSON_MALFORMED", "$", f"malformed JSON at {path}: {exc}")
        ) from exc
    if not isinstance(payload, dict):
        raise MatrixValidationError(
            _diag("M028_MATRIX_JSON_MALFORMED", "$", "matrix root must be an object")
        )
    return payload


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--matrix",
        type=Path,
        default=DEFAULT_MATRIX,
        help="Path to m028_requirement_scope_matrix.json",
    )
    parser.add_argument(
        "--rendered", type=Path, default=DEFAULT_RENDERED, help="Path to rendered matrix markdown"
    )
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument(
        "--require-requirements", nargs="+", default=sorted(REQUIRED_REQUIREMENT_IDS)
    )
    parser.add_argument(
        "--validate-only",
        action="store_true",
        help="Validate artifacts without generating or mutating outputs.",
    )
    parser.add_argument("--reject-unsafe-claims", action="store_true", default=True)
    parser.add_argument(
        "--require-planning-evidence",
        action="store_true",
        help="Also require .gsd/.planning/.audits evidence paths to exist; default skips these gitignored planning paths.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        matrix = _load_json(args.matrix)
        try:
            rendered = args.rendered.read_text(encoding="utf-8")
        except FileNotFoundError as exc:
            raise MatrixValidationError(
                _diag(
                    "M028_MATRIX_MARKDOWN_STALE",
                    "$rendered",
                    f"rendered markdown file not found: {args.rendered}",
                )
            ) from exc
        errors = validate_matrix(
            matrix,
            rendered,
            repo_root=args.repo_root,
            required_requirements=set(args.require_requirements),
            reject_unsafe_claims=args.reject_unsafe_claims,
            require_planning_evidence=args.require_planning_evidence,
        )
    except MatrixValidationError as exc:
        sys.stderr.write(f"ERROR: {exc}\n")
        return 2
    if errors:
        sys.stderr.write("M028 requirement scope reconciliation validation failed:\n")
        for error in errors:
            sys.stderr.write(f"- {error}\n")
        return 1
    sys.stdout.write(
        "M028 requirement scope reconciliation validation passed: "
        f"{len(matrix['requirements'])} requirement rows checked.\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
