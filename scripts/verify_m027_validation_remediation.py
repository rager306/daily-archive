#!/usr/bin/env python3
"""Validate the M027 S08 validation remediation class-audit package.

This verifier is intentionally read-only. It checks the canonical validation-class
audit JSON and rendered markdown against the M027 requirement scope matrix,
roadmap success criteria, and S07 validation evidence so milestone validation can
rerun with deterministic metadata-only diagnostics.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Iterable
from pathlib import Path
from typing import Any

MILESTONE_ID = "M027-aakeky"
SLICE_ID = "S08"
TASK_ID = "T03"
SCHEMA_VERSION = "m027-validation-remediation-class-audit.v1"
MATRIX_SCHEMA_VERSION = "m027-requirement-scope-matrix.v1"
MATRIX_PATH = "doc/validation/m027_requirement_scope_matrix.json"
MATRIX_MARKDOWN_PATH = "doc/validation/m027_requirement_scope_matrix.md"
# Flat-phase layout (nested .gsd/milestones/M027-... removed by GSD migration).
ROADMAP_PATH = (
    ".gsd/phases/27-aakeky-m027-aakeky-mixed-source-article-pipelin/27-ROADMAP.md"
)
AUDIT_JSON_PATH = "doc/validation/m027_validation_remediation_class_audit.json"
AUDIT_MARKDOWN_PATH = "doc/validation/m027_validation_remediation_class_audit.md"
S07_SUMMARY_PATH = "data/article_corpora/m027-mixed-source-corpus-v1/pipeline-readiness-synthesis/pipeline-readiness-synthesis-summary.json"
S07_REPORT_PATH = "data/article_corpora/m027-mixed-source-corpus-v1/pipeline-readiness-synthesis/pipeline-readiness-synthesis-report.md"
S07_DIAGNOSTICS_PATH = "data/article_corpora/m027-mixed-source-corpus-v1/pipeline-readiness-synthesis/pipeline-readiness-synthesis-diagnostics.jsonl"
S06_SUMMARY_PATH = "data/article_corpora/m027-mixed-source-corpus-v1/provenance-riskratchet-gate/provenance-riskratchet-gate-summary.json"

CANONICAL_CLASSES = ("Contract", "Integration", "Operational", "UAT")
M027_ADVANCED_REQUIREMENT_IDS = {"R024", "R027", "R029"}
FUTURE_OUT_OF_SCOPE_REQUIREMENT_IDS = {"R019", "R022", "R023", "R031", "R032", "R033"}
CHAIN_REQUIREMENT_IDS = {"R036"}
REQUIRED_REQUIREMENT_IDS = (
    M027_ADVANCED_REQUIREMENT_IDS | FUTURE_OUT_OF_SCOPE_REQUIREMENT_IDS | CHAIN_REQUIREMENT_IDS
)

EXPECTED_CLASSIFICATIONS: dict[str, dict[str, str]] = {
    **{
        rid: {
            "current_status": "active",
            "m027_applicability": "m027_advanced_preprocessing_only",
            "s08_verdict": "advanced_not_globally_validated",
            "recommended_requirement_action": "remain_active_with_m027_evidence_note",
        }
        for rid in sorted(M027_ADVANCED_REQUIREMENT_IDS)
    },
    **{
        rid: {
            "current_status": "active",
            "m027_applicability": "future_out_of_scope_active_requirement",
            "s08_verdict": "not_advanced_not_validated",
            "recommended_requirement_action": "remain_active",
        }
        for rid in sorted(FUTURE_OUT_OF_SCOPE_REQUIREMENT_IDS)
    },
    "R036": {
        "current_status": "active",
        "m027_applicability": "s07_closeout_validation_chain_preserved",
        "s08_verdict": "validation_chain_preserved_not_globally_closed",
        "recommended_requirement_action": "preserve_s07_closeout_chain_and_keep_active",
    },
}

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
    "scope_matrix_reference",
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
    "network_fetch_attempted",
    "runtime_surface_changed",
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
    "production_import_attempted",
    "ladybugdb_written",
    "trusted_fact_promotion_allowed",
    "graph_import_allowed",
}

UNSAFE_TRUE_BOOLEAN_KEYS = REQUIRED_FALSE_SAFETY_FLAGS | {
    "kg_import_ready",
    "import_ready",
    "import_eligible",
    "production_ladybugdb_write_allowed",
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
    "m027 validates graph readiness",
    "m027 authorizes kg import",
    "m027 authorizes kg import readiness",
    "m027 validates kg import readiness",
    "m027 validates scientific kg corpus",
    "m027 validates import-ready chunks",
    "m027 validates import ready chunks",
    "m027 promotes trusted facts",
    "m027 writes to production ladybugdb",
    "m027 activates dspy",
    "m027 activates rlm",
    "m027 activates minimax",
    "m027 activates optimizer",
    "m027 proves unattended scaling",
    "m027 embeds raw article text",
    "m027 embeds raw pdfs",
    "m027 embeds pdf bytes",
    "m027 embeds binary payloads",
    "m027 embeds base64 payloads",
    "m027 embeds vector payloads",
    "m027 embeds secrets",
    "m027 globally validates r024",
    "m027 globally validates r027",
    "m027 globally validates r029",
    "m027 fully validates r019",
    "m027 fully validates r022",
    "m027 fully validates r023",
    "m027 fully validates r024",
    "m027 fully validates r027",
    "m027 fully validates r029",
    "m027 fully validates r031",
    "m027 fully validates r032",
    "m027 fully validates r033",
    "m027 fully validates r036",
}

POSITIVE_TEXT_KEYS = {
    "criteria_source_decision",
    "roadmap_success_criteria",
    "interpretation",
    "supported_claims",
    "validation_recommendation",
    "scope",
    "planned_check",
    "safe_claim",
    "remaining_work",
    "safe_validation_wording",
    "observability_impact",
}

ALLOWED_EVIDENCE_SUFFIXES = {".json", ".jsonl", ".md", ".txt", ".yaml", ".yml", ".py"}
PLANNING_PATH_PREFIXES = (".gsd/", ".planning/", ".audits/")


class AuditValidationError(RuntimeError):
    """Raised for unreadable verifier inputs rather than audit contract failures."""


def _json_path(parent: str, key: str | int) -> str:
    return f"{parent}[{key}]" if isinstance(key, int) else f"{parent}.{key}"


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
        raise AuditValidationError(f"{label} file not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise AuditValidationError(f"malformed JSON in {label} at {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise AuditValidationError(f"{label} root must be an object: {path}")
    return payload


def _read_text(path: Path, label: str) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError as exc:
        raise AuditValidationError(f"{label} file not found: {path}") from exc


def _field_name_from_path(path: str) -> str:
    return path.rsplit(".", maxsplit=1)[-1].split("[", maxsplit=1)[0].lower()


def _is_planning_path(path: str) -> bool:
    return path.replace("\\", "/").startswith(PLANNING_PATH_PREFIXES)


def _validate_repo_path(
    path_value: Any, owner: str, *, repo_root: Path, require_planning_evidence: bool
) -> list[str]:
    if not isinstance(path_value, str):
        return [f"{owner} path must be a string: {path_value!r}"]
    if not path_value or path_value.strip() != path_value:
        return [f"{owner} path is blank or padded: {path_value!r}"]
    if "://" in path_value:
        return [f"{owner} path must be repo-relative, not a URL: {path_value}"]
    path = Path(path_value)
    if path.is_absolute():
        return [f"{owner} path must be relative: {path_value}"]
    if any(part == ".." for part in path.parts):
        return [f"{owner} path must not escape the repo: {path_value}"]
    if path.suffix and path.suffix not in ALLOWED_EVIDENCE_SUFFIXES:
        return [f"{owner} path has unsupported extension: {path_value}"]
    if _is_planning_path(path_value) and not require_planning_evidence:
        return []
    resolved = (repo_root / path).resolve()
    try:
        resolved.relative_to(repo_root.resolve())
    except ValueError:
        return [f"{owner} path escapes repo root: {path_value}"]
    if not resolved.exists():
        return [f"{owner} path does not exist: {path_value}"]
    return []


def _rows_by_id(rows: Any, key: str, base_path: str) -> tuple[dict[str, dict[str, Any]], list[str]]:
    if not isinstance(rows, list):
        return {}, [f"{base_path} must be a list"]
    by_id: dict[str, dict[str, Any]] = {}
    counts: dict[str, int] = {}
    errors: list[str] = []
    for index, row in enumerate(rows):
        row_path = f"{base_path}[{index}]"
        if not isinstance(row, dict):
            errors.append(f"{row_path} must be an object")
            continue
        row_id = row.get(key)
        if not isinstance(row_id, str) or not row_id:
            errors.append(f"{row_path}.{key} must be a non-empty string")
            continue
        by_id[row_id] = row  # ty:ignore[invalid-assignment]
        counts[row_id] = counts.get(row_id, 0) + 1
    duplicates = sorted(row_id for row_id, count in counts.items() if count > 1)
    if duplicates:
        errors.append(f"{base_path} has duplicate {key} values: {', '.join(duplicates)}")
    return by_id, errors


def _positive_text_for_audit(audit: dict[str, Any]) -> dict[str, str]:
    positive: dict[str, str] = {}
    for key in POSITIVE_TEXT_KEYS:
        if key in audit:
            positive[f"$.{key}"] = "\n".join(_strings_from(audit.get(key)))
    criteria = audit.get("criteria_source")
    if isinstance(criteria, dict):
        positive["$.criteria_source"] = "\n".join(
            _strings_from(
                {
                    "criteria_source_decision": criteria.get("criteria_source_decision"),
                    "roadmap_success_criteria": criteria.get("roadmap_success_criteria"),
                }
            )
        )
    coverage = audit.get("requirement_coverage_interpretation")
    if isinstance(coverage, dict):
        positive["$.requirement_coverage_interpretation"] = "\n".join(
            _strings_from(
                {
                    "interpretation": coverage.get("interpretation"),
                    "supported_claims": coverage.get("supported_claims"),
                }
            )
        )
    if "remaining_work" in audit:
        positive["$.remaining_work"] = "\n".join(_strings_from(audit.get("remaining_work")))
    for index, row in enumerate(audit.get("canonical_verification_classes", [])):
        if isinstance(row, dict):
            class_name = row.get("class", index)
            positive[f"$.canonical_verification_classes[{index}]({class_name})"] = "\n".join(
                _strings_from({key: row.get(key) for key in POSITIVE_TEXT_KEYS if key in row})
            )
    return positive


def validate_audit(
    audit: dict[str, Any],
    rendered_markdown: str,
    matrix: dict[str, Any],
    matrix_markdown: str,
    roadmap_markdown: str,
    s07_summary: dict[str, Any],
    s07_report: str,
    *,
    repo_root: Path | None = None,
    require_pass_classes: bool = True,
    reject_unsafe_claims: bool = True,
    require_planning_evidence: bool = False,
) -> list[str]:
    """Return deterministic diagnostics for the M027 class-audit package."""

    repo_root = repo_root or Path.cwd()
    errors: list[str] = []

    missing_top = sorted(REQUIRED_TOP_LEVEL_KEYS - set(audit))
    if missing_top:
        errors.append(f"$ missing top-level keys: {', '.join(missing_top)}")
    if audit.get("schema_version") != SCHEMA_VERSION:
        errors.append(f"$.schema_version must be {SCHEMA_VERSION}")
    if audit.get("milestone_id") != MILESTONE_ID:
        errors.append(f"$.milestone_id must be {MILESTONE_ID}")
    if audit.get("slice_id") != SLICE_ID:
        errors.append(f"$.slice_id must be {SLICE_ID}")
    if audit.get("task_id") != TASK_ID:
        errors.append(f"$.task_id must be {TASK_ID}")
    if audit.get("metadata_only") is not True:
        errors.append("$.metadata_only must be true")

    if matrix.get("schema_version") != MATRIX_SCHEMA_VERSION:
        errors.append(f"matrix schema_version must be {MATRIX_SCHEMA_VERSION}")
    if matrix.get("milestone_id") != MILESTONE_ID or matrix.get("slice_id") != SLICE_ID:
        errors.append("matrix milestone_id/slice_id must match the M027 S08 audit")
    if matrix.get("metadata_only") is not True:
        errors.append("matrix metadata_only must be true")

    expected_paths = {
        "$.criteria_source.canonical_success_criteria_source": ROADMAP_PATH,
        "$.scope_matrix_reference.source": MATRIX_PATH,
        "$.scope_matrix_reference.markdown": MATRIX_MARKDOWN_PATH,
        "$.rerun_ready_validation_inputs.success_criteria_checklist_source": ROADMAP_PATH,
        "$.rerun_ready_validation_inputs.requirement_coverage_source": MATRIX_PATH,
        "$.rerun_ready_validation_inputs.class_audit_source": AUDIT_JSON_PATH,
        "$.remediation_target.requirement_scope_matrix": MATRIX_PATH,
        "$.remediation_target.pipeline_readiness_synthesis_summary": S07_SUMMARY_PATH,
    }
    for path, expected in expected_paths.items():
        value: Any = audit
        for part in path.removeprefix("$.").split("."):
            value = value.get(part) if isinstance(value, dict) else None
        if value != expected:
            errors.append(f"{path} must be {expected!r}, found {value!r}")

    source_inputs = audit.get("source_inputs")
    required_inputs = {
        ROADMAP_PATH,
        MATRIX_PATH,
        MATRIX_MARKDOWN_PATH,
        S07_SUMMARY_PATH,
        S07_REPORT_PATH,
        S07_DIAGNOSTICS_PATH,
        S06_SUMMARY_PATH,
    }
    if not isinstance(source_inputs, list) or not all(
        isinstance(item, str) for item in source_inputs
    ):
        errors.append("$.source_inputs must be a list of repo-relative paths")
    else:
        missing_inputs = sorted(required_inputs - set(source_inputs))
        if missing_inputs:
            errors.append(f"$.source_inputs missing required inputs: {', '.join(missing_inputs)}")
        for path_value in source_inputs:
            errors.extend(
                _validate_repo_path(
                    path_value,
                    "$.source_inputs",
                    repo_root=repo_root,
                    require_planning_evidence=require_planning_evidence,
                )
            )

    flags = audit.get("safety_flags")
    if not isinstance(flags, dict):
        errors.append("$.safety_flags must be an object")
    else:
        missing_flags = sorted((REQUIRED_FALSE_SAFETY_FLAGS | {"metadata_only"}) - set(flags))
        if missing_flags:
            errors.append(f"$.safety_flags missing required fields: {', '.join(missing_flags)}")
        if flags.get("metadata_only") is not True:
            errors.append("$.safety_flags.metadata_only must be true")
        for key in sorted(REQUIRED_FALSE_SAFETY_FLAGS & set(flags)):
            if flags.get(key) is not False:
                errors.append(f"$.safety_flags.{key} must be false")

    matrix_flags = matrix.get("safety_flags")
    if isinstance(flags, dict) and isinstance(matrix_flags, dict):
        for key in sorted(set(flags) & set(matrix_flags)):
            if flags[key] != matrix_flags[key]:
                errors.append(f"$.safety_flags.{key} must match matrix safety flag {key}")

    matrix_rows, row_errors = _rows_by_id(
        matrix.get("requirements"), "requirement_id", "matrix.requirements"
    )
    errors.extend(row_errors)
    audit_rows, audit_row_errors = _rows_by_id(
        audit.get("requirement_coverage_interpretation", {}).get("requirement_rows")
        if isinstance(audit.get("requirement_coverage_interpretation"), dict)
        else None,
        "requirement_id",
        "$.requirement_coverage_interpretation.requirement_rows",
    )
    errors.extend(audit_row_errors)
    if set(audit_rows) != REQUIRED_REQUIREMENT_IDS:
        errors.append(
            "$.requirement_coverage_interpretation.requirement_rows must contain exactly: "
            + ", ".join(sorted(REQUIRED_REQUIREMENT_IDS))
        )
    for rid, expected in EXPECTED_CLASSIFICATIONS.items():
        for rows, label in ((matrix_rows, "matrix"), (audit_rows, "audit")):
            row = rows.get(rid)
            if row is None:
                continue
            for key, expected_value in expected.items():
                if row.get(key) != expected_value:
                    errors.append(
                        f"{label} {rid} {key} must be {expected_value}, found {row.get(key)!r}"
                    )
        if rid in matrix_rows and rid in audit_rows:
            for key in (
                "current_status",
                "m027_applicability",
                "s08_verdict",
                "recommended_requirement_action",
            ):
                if audit_rows[rid].get(key) != matrix_rows[rid].get(key):
                    errors.append(f"audit {rid} {key} must match matrix")

    class_rows, class_errors = _rows_by_id(
        audit.get("canonical_verification_classes"), "class", "$.canonical_verification_classes"
    )
    errors.extend(class_errors)
    actual_classes = set(class_rows)
    expected_classes = set(CANONICAL_CLASSES)
    if actual_classes != expected_classes:
        missing = sorted(expected_classes - actual_classes)
        extra = sorted(actual_classes - expected_classes)
        if missing:
            errors.append(
                f"$.canonical_verification_classes missing canonical classes: {', '.join(missing)}"
            )
        if extra:
            errors.append(
                f"$.canonical_verification_classes has unexpected classes: {', '.join(extra)}"
            )
    if isinstance(audit.get("canonical_verification_classes"), list) and [
        row.get("class") for row in audit["canonical_verification_classes"] if isinstance(row, dict)
    ] != list(CANONICAL_CLASSES):
        errors.append(
            "$.canonical_verification_classes must be ordered Contract, Integration, Operational, UAT"
        )

    for class_name in CANONICAL_CLASSES:
        row = class_rows.get(class_name)
        if row is None:
            continue
        row_path = f"$.canonical_verification_classes[{class_name}]"
        if require_pass_classes and row.get("verdict") != "PASS":
            errors.append(f"{row_path}.verdict must be PASS under --require-pass-classes")
        for key in ("scope", "planned_check", "safe_claim"):
            if not isinstance(row.get(key), str) or not row.get(key):
                errors.append(f"{row_path}.{key} must be a non-empty string")
        scope_text = "\n".join(
            _strings_from({k: row.get(k) for k in ("scope", "planned_check", "safe_claim")})
        ).lower()
        if "metadata-only" not in scope_text and "metadata only" not in scope_text:
            errors.append(f"{row_path} must constrain PASS semantics to metadata-only evidence")
        if "m027" not in scope_text or "s08" not in scope_text:
            errors.append(f"{row_path} must name the M027 S08 remediation boundary")
        evidence_paths = row.get("evidence_paths")
        if not isinstance(evidence_paths, list) or not evidence_paths:
            errors.append(f"{row_path}.evidence_paths must be a non-empty list")
        else:
            if MATRIX_PATH not in evidence_paths or AUDIT_JSON_PATH not in evidence_paths:
                errors.append(f"{row_path}.evidence_paths must include matrix and class audit JSON")
            for evidence in evidence_paths:
                errors.extend(
                    _validate_repo_path(
                        evidence,
                        f"{row_path}.evidence_paths",
                        repo_root=repo_root,
                        require_planning_evidence=require_planning_evidence,
                    )
                )
        if not isinstance(row.get("must_not_claim"), list) or not row.get("must_not_claim"):
            errors.append(f"{row_path}.must_not_claim must be a non-empty list")

    rerun = audit.get("rerun_ready_validation_inputs")
    if not isinstance(rerun, dict):
        errors.append("$.rerun_ready_validation_inputs must be an object")
    else:
        verification_classes = rerun.get("verification_classes")
        rerun_rows, rerun_errors = _rows_by_id(
            verification_classes, "class", "$.rerun_ready_validation_inputs.verification_classes"
        )
        errors.extend(rerun_errors)
        if set(rerun_rows) != expected_classes:
            errors.append(
                "$.rerun_ready_validation_inputs.verification_classes must list exactly the canonical classes"
            )
        for class_name in CANONICAL_CLASSES:
            class_row = class_rows.get(class_name)
            rerun_row = rerun_rows.get(class_name)
            if class_row and rerun_row:
                if rerun_row.get("verdict") != class_row.get("verdict"):
                    errors.append(
                        f"rerun verification class {class_name} verdict must match audit row"
                    )
                evidence = rerun_row.get("evidence")
                if (
                    not isinstance(evidence, list)
                    or MATRIX_PATH not in evidence
                    or AUDIT_JSON_PATH not in evidence
                ):
                    errors.append(
                        f"rerun verification class {class_name} evidence must include matrix and audit JSON"
                    )
        commands = rerun.get("commands")
        if not isinstance(commands, list) or not commands:
            errors.append("$.rerun_ready_validation_inputs.commands must be a non-empty list")
        elif (
            "uv run python scripts/verify_m027_validation_remediation.py --validate-only"
            not in commands
        ):
            errors.append(
                "$.rerun_ready_validation_inputs.commands missing class-audit validate-only command"
            )

    criteria = audit.get("criteria_source")
    if isinstance(criteria, dict):
        criteria_text = "\n".join(_strings_from(criteria.get("roadmap_success_criteria")))
        for marker in (
            "six user-supplied mixed-source article URLs",
            "R036-style provenance",
            "preprocessing-only",
        ):
            if marker not in criteria_text and marker not in roadmap_markdown:
                errors.append(f"$.criteria_source must preserve roadmap criterion marker: {marker}")
    else:
        errors.append("$.criteria_source must be an object")

    if "M027-advanced but not globally validated" not in matrix_markdown:
        errors.append("matrix markdown missing M027 advanced/global validation distinction")
    if s07_summary.get("graph_import_allowed") is not False:
        errors.append("S07 summary graph_import_allowed must be false")
    if s07_summary.get("ladybugdb_written") is not False:
        errors.append("S07 summary ladybugdb_written must be false")
    if s07_summary.get("network_fetch_attempted") is not False:
        errors.append("S07 summary network_fetch_attempted must be false")
    if "not_import_ready_validate_only" not in s07_report:
        errors.append("S07 report must preserve not_import_ready_validate_only evidence")

    if reject_unsafe_claims:
        for path, text in _positive_text_for_audit(audit).items():
            lowered = text.lower()
            for phrase in sorted(UNSAFE_POSITIVE_CLAIM_PHRASES):
                if phrase in lowered:
                    errors.append(f"{path} contains unsafe positive claim phrase: {phrase}")

    for path, value in _walk(audit):
        field_name = _field_name_from_path(path)
        if isinstance(value, bool) and value is True and field_name in UNSAFE_TRUE_BOOLEAN_KEYS:
            errors.append(f"{path} unsafe boolean field must not be true")
        if any(fragment in field_name for fragment in UNSAFE_FIELD_NAME_FRAGMENTS):
            if not path.startswith("$.safety_flags."):
                errors.append(f"{path} contains unsafe raw/binary/base64/vector/secret field name")
        if isinstance(value, str):
            lowered = value.lower()
            if (
                "-----begin" in lowered
                or "base64," in lowered
                or "secret=" in lowered
                or "password=" in lowered
            ):
                errors.append(f"{path} contains raw payload, base64, or secret leakage marker")

    if not rendered_markdown.strip():
        errors.append("rendered markdown is empty")
    for marker in (
        "# M027 Validation Remediation Class Audit",
        AUDIT_JSON_PATH,
        SCHEMA_VERSION,
        "metadata-only",
        "## Requirement Coverage Interpretation",
        "## Canonical Verification Classes",
        "## Rerun-Ready Validation Inputs",
        "## Forbidden Claims",
        "## Failure Modes",
        "## Load Profile",
        "## Negative Tests",
        "## Observability Impact",
    ):
        if marker not in rendered_markdown:
            errors.append(f"rendered markdown missing marker: {marker}")
    for class_name in CANONICAL_CLASSES:
        if f"| {class_name} | PASS |" not in rendered_markdown:
            errors.append(f"rendered markdown missing PASS class row for {class_name}")
    for rid, expected in EXPECTED_CLASSIFICATIONS.items():
        if rid not in rendered_markdown:
            errors.append(f"rendered markdown missing requirement id: {rid}")
        for expected_value in expected.values():
            if expected_value not in rendered_markdown:
                errors.append(f"rendered markdown missing {rid} expected value: {expected_value}")
    if (
        "uv run python scripts/verify_m027_validation_remediation.py --validate-only"
        not in rendered_markdown
    ):
        errors.append("rendered markdown missing validate-only rerun command")

    return errors


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--audit", type=Path, default=Path(AUDIT_JSON_PATH))
    parser.add_argument("--rendered", type=Path, default=Path(AUDIT_MARKDOWN_PATH))
    parser.add_argument("--matrix", type=Path, default=Path(MATRIX_PATH))
    parser.add_argument("--matrix-rendered", type=Path, default=Path(MATRIX_MARKDOWN_PATH))
    parser.add_argument("--roadmap", type=Path, default=Path(ROADMAP_PATH))
    parser.add_argument("--s07-summary", type=Path, default=Path(S07_SUMMARY_PATH))
    parser.add_argument("--s07-report", type=Path, default=Path(S07_REPORT_PATH))
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument(
        "--validate-only", action="store_true", help="Validate artifacts without mutating outputs."
    )
    parser.add_argument("--require-pass-classes", action="store_true", default=True)
    parser.add_argument("--reject-unsafe-claims", action="store_true", default=True)
    parser.add_argument(
        "--require-planning-evidence",
        action="store_true",
        help="Also require .gsd/.planning/.audits evidence paths to exist; default skips gitignored planning paths.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        audit = _load_json(args.audit, "class audit")
        rendered = _read_text(args.rendered, "rendered class audit")
        matrix = _load_json(args.matrix, "requirement scope matrix")
        matrix_markdown = _read_text(args.matrix_rendered, "rendered requirement scope matrix")
        roadmap = _read_text(args.roadmap, "roadmap")
        s07_summary = _load_json(args.s07_summary, "S07 pipeline readiness summary")
        s07_report = _read_text(args.s07_report, "S07 pipeline readiness report")
        errors = validate_audit(
            audit,
            rendered,
            matrix,
            matrix_markdown,
            roadmap,
            s07_summary,
            s07_report,
            repo_root=args.repo_root,
            require_pass_classes=args.require_pass_classes,
            reject_unsafe_claims=args.reject_unsafe_claims,
            require_planning_evidence=args.require_planning_evidence,
        )
    except AuditValidationError as exc:
        sys.stderr.write(f"ERROR: {exc}\n")
        return 2
    if errors:
        sys.stderr.write("M027 validation remediation class audit failed:\n")
        for error in errors:
            sys.stderr.write(f"- {error}\n")
        return 1
    sys.stdout.write(
        "M027 validation remediation class audit passed: "
        f"{len(audit['canonical_verification_classes'])} canonical classes and "
        f"{len(audit['requirement_coverage_interpretation']['requirement_rows'])} requirement rows checked.\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
