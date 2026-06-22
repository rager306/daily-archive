#!/usr/bin/env python3
"""Fail-closed verifier for the M029 validation-remediation dossier.

The verifier is intentionally read-only except for the optional metadata-only
``--write-verify-summary`` output. It validates local JSON/Markdown/JSONL
artifacts supplied as repo-relative paths and refuses validation overclaims while
M030 completion, M030/S06 output, and M030-derived M029 replan proof are absent.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path
from typing import Any

MILESTONE_ID = "M029-eb0ljz"
SLICE_ID = "S07"
TASK_ID = "T01"
SELECTION_ID = "m029-unified-corpus-v1"
SCHEMA_VERSION = "m029-validation-remediation-evidence.v1"
VERIFY_SCHEMA_VERSION = "m029-validation-remediation-verifier.v1"
EXPECTED_REQUIREMENT_IDS = ("R024", "R027", "R029", "R035", "R040", "R050")
BLOCKED_VERDICTS = {
    "blocked_pending_m030_completion",
    "blocked_pending_m030_completion_and_replan",
    "blocked_missing_m030_completion_and_s06_outputs",
    "remediation_required",
    "needs_remediation",
    "blocked",
}
REQUIRED_TOP_LEVEL_KEYS = {
    "schema_version",
    "artifact_version",
    "milestone_id",
    "slice_id",
    "task_id",
    "selection_id",
    "verdict",
    "verdict_reason",
    "prerequisite_audit",
    "replan_audit",
    "bounded_ref_reconciliation",
    "bounded_ref_counts",
    "provisional_m029_readiness_counts",
    "requirement_coverage",
    "safety_flags",
    "metadata_only_boundary",
    "safe_closeout_wording",
    "forbidden_claims",
    "diagnostic_codes",
    "diagnostic_count",
    "remaining_remediation_scope",
    "input_artifact_audit",
}
REQUIRED_FALSE_FLAGS = {
    "graph_import_attempted",
    "graph_write_attempted",
    "trusted_kg_import_allowed",
    "kg_readiness_claimed",
    "ladybugdb_written",
    "production_import_attempted",
    "production_persistence_attempted",
    "network_fetch_attempted",
    "source_loader_modified",
    "parser_or_chunker_modified",
    "catalog_registration_modified",
    "requirement_records_modified",
    "source_write_attempted",
    "non_artifact_write_attempted",
    "raw_article_text_embedded",
    "raw_pdf_bytes_embedded",
    "binary_payload_embedded",
    "vectors_embedded",
    "secrets_embedded",
}
REQUIRED_METADATA_FALSE_FLAGS = {
    "raw_article_text_included",
    "raw_pdf_bytes_included",
    "binary_payloads_included",
    "vectors_included",
    "secrets_included",
}
UNSAFE_TRUE_BOOLEAN_KEYS = (
    REQUIRED_FALSE_FLAGS
    | REQUIRED_METADATA_FALSE_FLAGS
    | {
        "validated",
        "validation_claim_allowed",
        "ready_for_graph_import",
        "graph_import_allowed",
        "trusted_fact_promotion_allowed",
        "production_import_allowed",
        "production_ladybugdb_write_allowed",
        "source_acquisition_completed",
        "parser_ready_claimed",
        "chunk_ready_claimed",
    }
)
UNSAFE_FIELD_NAME_FRAGMENTS = {
    "raw_article_text",
    "raw_text",
    "raw_pdf_bytes",
    "raw_payload",
    "binary_payload",
    "base64_payload",
    "vector_payload",
    "embedding_payload",
    "secret_value",
    "password",
    "api_key",
}
ALLOWED_FALSE_PAYLOAD_FIELD_ANCESTORS = ("$.safety_flags.", "$.metadata_only_boundary.")
FORBIDDEN_POSITIVE_PHRASES = {
    "m029 is validated",
    "m029 validation passed",
    "m029 is ready for graph import",
    "m029 completed the post-m030 replan",
    "m030 completed and produced s06 roadmap output",
    "all m030/s01 bounded refs are represented",
    "r024 is validated",
    "r027 is validated",
    "r029 is validated",
    "r035 is validated",
    "r040 is validated",
    "r050 is validated",
    "ladybugdb was written",
    "production import was attempted",
}
REQUIRED_DIAGNOSTIC_CODES = {
    "M029_REMEDIATION_MISSING_M030_COMPLETION",
    "M029_REMEDIATION_MISSING_M030_S06_ROADMAP_OUTPUT",
    "M029_REMEDIATION_MISSING_M029_REPLAN_PROOF",
    "M029_REMEDIATION_MISSING_BOUNDED_REF",
}
PATH_LIKE_KEYS = {
    "path",
    "m030_completion_artifact",
    "m030_s01_intake_summary",
    "m030_s06_summary",
    "m030_s06_uat",
    "runtime_summary_path",
    "summary_path",
    "decision_path",
    "report_path",
}
PATH_LIST_KEYS = {"candidate_replan_artifacts", "source_artifact_paths", "source_inputs"}


class VerifierInputError(RuntimeError):
    """Raised for missing or malformed verifier inputs."""


def _json_path(parent: str, key: str | int) -> str:
    return f"{parent}[{key}]" if isinstance(key, int) else f"{parent}.{key}"


def _walk(
    value: Any, path: str = "$", ancestors: tuple[str, ...] = ()
) -> Iterable[tuple[str, Any, tuple[str, ...]]]:
    yield path, value, ancestors
    if isinstance(value, dict):
        for key, child in value.items():
            yield from _walk(child, _json_path(path, str(key)), (*ancestors, str(key)))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from _walk(child, _json_path(path, index), ancestors)


def _diagnostic(
    code: str,
    message: str,
    *,
    json_path: str = "$",
    path: str | None = None,
    severity: str = "error",
) -> dict[str, Any]:
    return {
        "schema_version": VERIFY_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "severity": severity,
        "code": code,
        "diagnostic_code": code,
        "message": message,
        "json_path": json_path,
        "path": path,
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_write_attempted": False,
    }


def _repo_relative_path(
    path_value: str, *, repo_root: Path, label: str, require_exists: bool = True
) -> Path:
    if (
        not isinstance(path_value, str)
        or not path_value.strip()
        or path_value.strip() != path_value
    ):
        raise VerifierInputError(f"{label} must be a non-empty unpadded repo-relative path")
    if "://" in path_value:
        raise VerifierInputError(f"{label} must be a repo-relative path, not a URL: {path_value}")
    path = Path(path_value)
    if path.is_absolute() or any(part == ".." for part in path.parts):
        raise VerifierInputError(
            f"{label} must be a repo-relative path under the repo root: {path_value}"
        )
    root = repo_root.resolve()
    resolved = (root / path).resolve()
    try:
        resolved.relative_to(root)
    except ValueError as exc:
        raise VerifierInputError(f"{label} escapes repo root: {path_value}") from exc
    if require_exists and not resolved.exists():
        raise VerifierInputError(f"{label} file not found: {path_value}")
    return resolved


def _load_json_arg(path_value: str, *, repo_root: Path, label: str) -> dict[str, Any]:
    path = _repo_relative_path(path_value, repo_root=repo_root, label=label)
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise VerifierInputError(f"malformed JSON in {label} at {path_value}: {exc}") from exc
    if not isinstance(value, dict):
        raise VerifierInputError(f"{label} root must be a JSON object: {path_value}")
    return value


def _read_text_arg(path_value: str, *, repo_root: Path, label: str) -> str:
    path = _repo_relative_path(path_value, repo_root=repo_root, label=label)
    return path.read_text(encoding="utf-8")


def _read_jsonl_arg(path_value: str, *, repo_root: Path, label: str) -> list[dict[str, Any]]:
    path = _repo_relative_path(path_value, repo_root=repo_root, label=label)
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as exc:
            raise VerifierInputError(
                f"malformed JSONL in {label} at {path_value}:{line_number}: {exc}"
            ) from exc
        if not isinstance(value, dict):
            raise VerifierInputError(f"{label} row must be an object at {path_value}:{line_number}")
        value.setdefault("_line_number", line_number)
        rows.append(value)
    return rows


def _field_name(path: str) -> str:
    return path.rsplit(".", maxsplit=1)[-1].split("[", maxsplit=1)[0].lower()


def _validate_artifact_paths(evidence: Mapping[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    for path, value, ancestors in _walk(evidence):
        key = ancestors[-1] if ancestors else ""
        if key in PATH_LIKE_KEYS and value is not None:
            values = [value]
        elif key in PATH_LIST_KEYS and isinstance(value, list):
            values = value
        else:
            continue
        for item in values:
            if not isinstance(item, str) or not item:
                diagnostics.append(
                    _diagnostic(
                        "M029_REMEDIATION_UNSAFE_SOURCE_PATH",
                        f"source path must be a non-empty string: {item!r}",
                        json_path=path,
                    )
                )
                continue
            if (
                "://" in item
                or Path(item).is_absolute()
                or any(part == ".." for part in Path(item.replace("\\", "/")).parts)
            ):
                diagnostics.append(
                    _diagnostic(
                        "M029_REMEDIATION_UNSAFE_SOURCE_PATH",
                        f"source path must be relative and under repo root: {item}",
                        json_path=path,
                        path=item,
                    )
                )
    return diagnostics


def _validate_shape(evidence: Mapping[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    missing = sorted(REQUIRED_TOP_LEVEL_KEYS - set(evidence))
    if missing:
        diagnostics.append(
            _diagnostic(
                "M029_REMEDIATION_MISSING_TOP_LEVEL_KEY",
                "missing top-level keys: " + ", ".join(missing),
            )
        )
    expected = {
        "$.schema_version": ("schema_version", SCHEMA_VERSION),
        "$.milestone_id": ("milestone_id", MILESTONE_ID),
        "$.slice_id": ("slice_id", SLICE_ID),
        "$.task_id": ("task_id", TASK_ID),
        "$.selection_id": ("selection_id", SELECTION_ID),
    }
    for json_path, (key, expected_value) in expected.items():
        if evidence.get(key) != expected_value:
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_IDENTITY_MISMATCH",
                    f"{key} must be {expected_value}",
                    json_path=json_path,
                )
            )
    if evidence.get("artifact_version") != 1:
        diagnostics.append(
            _diagnostic(
                "M029_REMEDIATION_SCHEMA_MISMATCH",
                "artifact_version must be 1",
                json_path="$.artifact_version",
            )
        )
    return diagnostics


def _validate_prerequisites_and_verdict(evidence: Mapping[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    prereq = evidence.get("prerequisite_audit")
    replan = evidence.get("replan_audit")
    if not isinstance(prereq, Mapping):
        return [
            _diagnostic(
                "M029_REMEDIATION_MISSING_PREREQUISITE_AUDIT",
                "prerequisite_audit must be an object",
                json_path="$.prerequisite_audit",
            )
        ]
    if not isinstance(replan, Mapping):
        return [
            _diagnostic(
                "M029_REMEDIATION_MISSING_REPLAN_AUDIT",
                "replan_audit must be an object",
                json_path="$.replan_audit",
            )
        ]
    missing_flags = {
        "M029_REMEDIATION_MISSING_M030_COMPLETION": (
            "$.prerequisite_audit.m030_completion_artifact_present",
            prereq.get("m030_completion_artifact_present"),
            prereq.get("m030_completion_artifact"),
        ),
        "M029_REMEDIATION_MISSING_M030_S06_ROADMAP_OUTPUT": (
            "$.prerequisite_audit.m030_s06_summary_present",
            prereq.get("m030_s06_summary_present"),
            prereq.get("m030_s06_summary"),
        ),
        "M029_REMEDIATION_MISSING_M029_REPLAN_PROOF": (
            "$.replan_audit.m030_derived_m029_replan_proof_present",
            replan.get("m030_derived_m029_replan_proof_present"),
            "$.replan_audit.candidate_replan_artifacts",
        ),
    }
    prerequisites_missing = False
    for _code, (json_path, actual, artifact_path) in missing_flags.items():
        if actual is not False:
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_PREREQUISITE_OVERCLAIM",
                    f"missing prerequisite flag must remain false while proof is absent: {json_path}",
                    json_path=json_path,
                    path=str(artifact_path) if artifact_path is not None else None,
                )
            )
        else:
            prerequisites_missing = True
    if prerequisites_missing and str(evidence.get("verdict")) not in BLOCKED_VERDICTS:
        diagnostics.append(
            _diagnostic(
                "M029_REMEDIATION_FALSE_PASS_VERDICT",
                "verdict must remain blocked/remediation while prerequisites are missing",
                json_path="$.verdict",
            )
        )
    return diagnostics


def _selection_identity_set(selection: Mapping[str, Any], *, rows_key: str) -> set[str]:
    rows = selection.get(rows_key)
    if not isinstance(rows, list):
        return set()
    identities: set[str] = set()
    for row in rows:
        if isinstance(row, Mapping):
            value = row.get("identity_key") or row.get("normalized_identity")
            if isinstance(value, str) and value:
                identities.add(value)
    return identities


def _m030_refs(selection: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    refs = selection.get("refs")
    if not isinstance(refs, list):
        return {}
    by_id: dict[str, Mapping[str, Any]] = {}
    for row in refs:
        if isinstance(row, Mapping) and isinstance(row.get("ref_id"), str):
            by_id[str(row["ref_id"])] = row
    return by_id


def _validate_bounded_refs(
    evidence: Mapping[str, Any],
    m029_selection: Mapping[str, Any],
    m030_selection: Mapping[str, Any],
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    rows = evidence.get("bounded_ref_reconciliation")
    if not isinstance(rows, list):
        return [
            _diagnostic(
                "M029_REMEDIATION_MISSING_BOUNDED_REF_ROWS",
                "bounded_ref_reconciliation must be a list",
                json_path="$.bounded_ref_reconciliation",
            )
        ]
    row_by_ref = {row.get("bounded_ref_id"): row for row in rows if isinstance(row, Mapping)}
    refs = _m030_refs(m030_selection)
    m029_identities = _selection_identity_set(m029_selection, rows_key="articles")
    if set(row_by_ref) != set(refs):
        missing = sorted(set(refs) - set(row_by_ref))
        # pyrefly: ignore [bad-specialization]
        extra = sorted(set(row_by_ref) - set(refs))
        diagnostics.append(
            _diagnostic(
                "M029_REMEDIATION_BOUNDED_REF_MISMATCH",
                f"bounded refs must match M030/S01 refs; missing={missing}, extra={extra}",
                json_path="$.bounded_ref_reconciliation",
            )
        )
    missing_count = 0
    represented_count = 0
    for index, row in enumerate(rows):
        if not isinstance(row, Mapping):
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_MALFORMED_BOUNDED_REF_ROW",
                    "bounded ref row must be an object",
                    json_path=f"$.bounded_ref_reconciliation[{index}]",
                )
            )
            continue
        ref_id = row.get("bounded_ref_id")
        ref = refs.get(str(ref_id))
        if ref is None:
            continue
        identity = ref.get("normalized_identity")
        if row.get("normalized_identity") != identity:
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_BOUNDED_REF_MISMATCH",
                    "bounded ref normalized_identity does not match M030/S01 selection",
                    json_path=f"$.bounded_ref_reconciliation[{index}].normalized_identity",
                )
            )
        expected_present = isinstance(identity, str) and identity in m029_identities
        if row.get("present_in_provisional_m029_selection") is not expected_present:
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_BOUNDED_REF_MISMATCH",
                    "present_in_provisional_m029_selection does not match provisional M029 selection",
                    json_path=f"$.bounded_ref_reconciliation[{index}].present_in_provisional_m029_selection",
                )
            )
        if expected_present:
            represented_count += 1
            if row.get("reconciliation_status") != "represented_in_provisional_m029_corpus":
                diagnostics.append(
                    _diagnostic(
                        "M029_REMEDIATION_BOUNDED_REF_MISMATCH",
                        "represented bounded ref has wrong reconciliation_status",
                        json_path=f"$.bounded_ref_reconciliation[{index}].reconciliation_status",
                    )
                )
        else:
            missing_count += 1
            if row.get("reconciliation_status") != "missing_from_provisional_m029_corpus":
                diagnostics.append(
                    _diagnostic(
                        "M029_REMEDIATION_BOUNDED_REF_MISMATCH",
                        "missing bounded ref has wrong reconciliation_status",
                        json_path=f"$.bounded_ref_reconciliation[{index}].reconciliation_status",
                    )
                )
            if row.get("safe_next_action") != "add_to_post_m030_replan_scope_before_validation":
                diagnostics.append(
                    _diagnostic(
                        "M029_REMEDIATION_MISSING_BOUNDED_REF",
                        "missing bounded ref must be carried into post-M030 replan scope",
                        json_path=f"$.bounded_ref_reconciliation[{index}].safe_next_action",
                    )
                )
    counts = evidence.get("bounded_ref_counts")
    if not isinstance(counts, Mapping):
        diagnostics.append(
            _diagnostic(
                "M029_REMEDIATION_BOUNDED_REF_COUNT_DRIFT",
                "bounded_ref_counts must be an object",
                json_path="$.bounded_ref_counts",
            )
        )
    else:
        expected_counts = {
            "m030_s01_bounded_ref_count": len(refs),
            "missing_from_provisional_m029_count": missing_count,
            "represented_in_provisional_m029_count": represented_count,
        }
        for key, expected in expected_counts.items():
            if counts.get(key) != expected:
                diagnostics.append(
                    _diagnostic(
                        "M029_REMEDIATION_BOUNDED_REF_COUNT_DRIFT",
                        f"{key} must be {expected}",
                        json_path=f"$.bounded_ref_counts.{key}",
                    )
                )
    if missing_count and "M029_REMEDIATION_MISSING_BOUNDED_REF" not in set(
        evidence.get("diagnostic_codes", [])
    ):
        diagnostics.append(
            _diagnostic(
                "M029_REMEDIATION_DIAGNOSTIC_DRIFT",
                "missing bounded refs require M029_REMEDIATION_MISSING_BOUNDED_REF diagnostic code",
                json_path="$.diagnostic_codes",
            )
        )
    return diagnostics


def _validate_readiness_counts(
    evidence: Mapping[str, Any], readiness: Mapping[str, Any]
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    counts = evidence.get("provisional_m029_readiness_counts")
    if not isinstance(counts, Mapping):
        return [
            _diagnostic(
                "M029_REMEDIATION_READINESS_COUNT_DRIFT",
                "provisional_m029_readiness_counts must be an object",
                json_path="$.provisional_m029_readiness_counts",
            )
        ]
    for key in (
        "status",
        "article_count",
        "decision",
        "ready_count",
        "zero_chunk_count",
        "unsafe_flag_count",
    ):
        if counts.get(key) != readiness.get(key):
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_READINESS_COUNT_DRIFT",
                    f"{key} must match readiness verifier summary",
                    json_path=f"$.provisional_m029_readiness_counts.{key}",
                )
            )
    if counts.get("status") != "passed" or counts.get("unsafe_flag_count") != 0:
        diagnostics.append(
            _diagnostic(
                "M029_REMEDIATION_READINESS_COUNT_DRIFT",
                "readiness evidence must remain passed with zero unsafe flags",
                json_path="$.provisional_m029_readiness_counts",
            )
        )
    return diagnostics


def _validate_safety_and_raw_fields(evidence: Mapping[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    flags = evidence.get("safety_flags")
    boundary = evidence.get("metadata_only_boundary")
    if not isinstance(flags, Mapping):
        diagnostics.append(
            _diagnostic(
                "M029_REMEDIATION_UNSAFE_FLAG_TRUE",
                "safety_flags must be an object",
                json_path="$.safety_flags",
            )
        )
    else:
        for key in sorted(REQUIRED_FALSE_FLAGS):
            if flags.get(key) is not False:
                diagnostics.append(
                    _diagnostic(
                        "M029_REMEDIATION_UNSAFE_FLAG_TRUE",
                        f"safety flag must be false: {key}",
                        json_path=f"$.safety_flags.{key}",
                    )
                )
    if not isinstance(boundary, Mapping):
        diagnostics.append(
            _diagnostic(
                "M029_REMEDIATION_UNSAFE_FLAG_TRUE",
                "metadata_only_boundary must be an object",
                json_path="$.metadata_only_boundary",
            )
        )
    else:
        if boundary.get("relative_paths_only") is not True:
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_UNSAFE_SOURCE_PATH",
                    "metadata_only_boundary.relative_paths_only must be true",
                    json_path="$.metadata_only_boundary.relative_paths_only",
                )
            )
        for key in sorted(REQUIRED_METADATA_FALSE_FLAGS):
            if boundary.get(key) is not False:
                diagnostics.append(
                    _diagnostic(
                        "M029_REMEDIATION_RAW_PAYLOAD_FIELD",
                        f"metadata-only boundary flag must be false: {key}",
                        json_path=f"$.metadata_only_boundary.{key}",
                    )
                )
    for path, value, _ancestors in _walk(evidence):
        field = _field_name(path)
        if isinstance(value, bool) and value is True and field in UNSAFE_TRUE_BOOLEAN_KEYS:
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_UNSAFE_FLAG_TRUE",
                    f"unsafe boolean field must not be true: {field}",
                    json_path=path,
                )
            )
        if any(fragment in field for fragment in UNSAFE_FIELD_NAME_FRAGMENTS):
            allowed_false_boundary = (
                path.startswith(ALLOWED_FALSE_PAYLOAD_FIELD_ANCESTORS) and value is False
            )
            if not allowed_false_boundary:
                diagnostics.append(
                    _diagnostic(
                        "M029_REMEDIATION_RAW_PAYLOAD_FIELD",
                        f"raw/binary/vector/secret field is not allowed outside false safety metadata: {field}",
                        json_path=path,
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
                diagnostics.append(
                    _diagnostic(
                        "M029_REMEDIATION_RAW_PAYLOAD_FIELD",
                        "string contains raw payload/base64/secret marker",
                        json_path=path,
                    )
                )
    return diagnostics


def _validate_requirements(evidence: Mapping[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    rows = evidence.get("requirement_coverage")
    if not isinstance(rows, list):
        return [
            _diagnostic(
                "M029_REMEDIATION_REQUIREMENT_OVERCLAIM",
                "requirement_coverage must be a list",
                json_path="$.requirement_coverage",
            )
        ]
    by_id = {row.get("requirement_id"): row for row in rows if isinstance(row, Mapping)}
    if set(by_id) != set(EXPECTED_REQUIREMENT_IDS):
        diagnostics.append(
            _diagnostic(
                "M029_REMEDIATION_REQUIREMENT_OVERCLAIM",
                "requirement_coverage must contain exactly " + ", ".join(EXPECTED_REQUIREMENT_IDS),
                json_path="$.requirement_coverage",
            )
        )
    for index, row in enumerate(rows):
        if not isinstance(row, Mapping):
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_REQUIREMENT_OVERCLAIM",
                    "requirement row must be an object",
                    json_path=f"$.requirement_coverage[{index}]",
                )
            )
            continue
        rid = row.get("requirement_id")
        if row.get("coverage_status") != "scoped_for_remediation_only":
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_REQUIREMENT_OVERCLAIM",
                    f"{rid} coverage_status must remain scoped_for_remediation_only",
                    json_path=f"$.requirement_coverage[{index}].coverage_status",
                )
            )
        if row.get("validated") is not False:
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_REQUIREMENT_STATUS_OVERCLAIM",
                    f"{rid} must not claim validated",
                    json_path=f"$.requirement_coverage[{index}].validated",
                )
            )
        if row.get("validation_claim_allowed") is not False:
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_REQUIREMENT_STATUS_OVERCLAIM",
                    f"{rid} validation claim must not be allowed",
                    json_path=f"$.requirement_coverage[{index}].validation_claim_allowed",
                )
            )
    return diagnostics


def _report_without_forbidden_section(report: str) -> str:
    lowered = report.lower()
    marker = "\n## forbidden claims"
    index = lowered.find(marker)
    if index == -1:
        return report
    next_index = lowered.find("\n## ", index + len(marker))
    if next_index == -1:
        return report[:index]
    return report[:index] + report[next_index:]


def _validate_report(evidence: Mapping[str, Any], report: str) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    required_markers = [
        "# M029 Validation Remediation Dossier",
        str(evidence.get("verdict")),
        "## Prerequisite and Replan Status",
        "## M030/S01 Bounded-Ref Reconciliation",
        "## Provisional S06 Readiness",
        "## Requirement Coverage Narrowing",
        "## Safety Flags",
        "## Stable Diagnostics",
        "## Safe Closeout Wording",
        "## Forbidden Claims",
        "## Remaining Remediation Scope",
    ]
    for marker in required_markers:
        if marker and marker not in report:
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_REPORT_SYNC_DRIFT",
                    f"report missing marker: {marker}",
                    path="report",
                )
            )
    readiness_counts = (
        evidence.get("provisional_m029_readiness_counts", {})
        if isinstance(evidence.get("provisional_m029_readiness_counts"), Mapping)
        else {}
    )
    for key in (
        "status",
        "article_count",
        "ready_count",
        "zero_chunk_count",
        "unsafe_flag_count",
        "decision",
    ):
        value = readiness_counts.get(key)
        if isinstance(value, (str, int)) and str(value) not in report:
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_REPORT_SYNC_DRIFT",
                    f"report missing readiness value: {value}",
                    path="report",
                )
            )
    positive_text = _report_without_forbidden_section(report).lower()
    for phrase in sorted(FORBIDDEN_POSITIVE_PHRASES):
        if phrase in positive_text:
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_FORBIDDEN_POSITIVE_CLAIM",
                    f"report contains forbidden positive claim phrase: {phrase}",
                    path="report",
                )
            )
    for snippet in (
        "<html",
        "<!doctype html",
        "%pdf-",
        "base64,",
        "secret=",
        "password=",
        "-----begin",
    ):
        if snippet in report.lower():
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_RAW_PAYLOAD_FIELD",
                    f"report contains forbidden payload marker: {snippet}",
                    path="report",
                )
            )
    return diagnostics


def _validate_diagnostics(
    evidence: Mapping[str, Any], diagnostic_rows: Sequence[Mapping[str, Any]]
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    if evidence.get("diagnostic_count") != len(diagnostic_rows):
        diagnostics.append(
            _diagnostic(
                "M029_REMEDIATION_DIAGNOSTIC_DRIFT",
                "diagnostic_count does not match JSONL rows",
                json_path="$.diagnostic_count",
            )
        )
    codes = [str(row.get("code") or row.get("diagnostic_code") or "") for row in diagnostic_rows]
    if set(evidence.get("diagnostic_codes", [])) != set(codes):
        diagnostics.append(
            _diagnostic(
                "M029_REMEDIATION_DIAGNOSTIC_DRIFT",
                "diagnostic_codes do not match JSONL rows",
                json_path="$.diagnostic_codes",
            )
        )
    missing_required = sorted(REQUIRED_DIAGNOSTIC_CODES - set(codes))
    if missing_required:
        diagnostics.append(
            _diagnostic(
                "M029_REMEDIATION_DIAGNOSTIC_DRIFT",
                "missing required diagnostic codes: " + ", ".join(missing_required),
                json_path="diagnostics.jsonl",
            )
        )
    for index, row in enumerate(diagnostic_rows):
        row_path = f"diagnostics.jsonl:{row.get('_line_number', index + 1)}"
        for key in ("code", "severity", "json_path", "message"):
            if not row.get(key):
                diagnostics.append(
                    _diagnostic(
                        "M029_REMEDIATION_DIAGNOSTIC_DRIFT",
                        f"diagnostic row missing {key}",
                        json_path=row_path,
                    )
                )
        if row.get("severity") != "blocking":
            diagnostics.append(
                _diagnostic(
                    "M029_REMEDIATION_DIAGNOSTIC_DRIFT",
                    "diagnostic severity must remain blocking",
                    json_path=row_path,
                )
            )
    return diagnostics


def validate_remediation(
    evidence: Mapping[str, Any],
    report: str,
    diagnostic_rows: Sequence[Mapping[str, Any]],
    m029_selection: Mapping[str, Any],
    m030_selection: Mapping[str, Any],
    readiness_verify: Mapping[str, Any],
) -> list[dict[str, Any]]:
    """Return fail-closed diagnostics for the M029 remediation dossier."""

    diagnostics: list[dict[str, Any]] = []
    diagnostics.extend(_validate_shape(evidence))
    diagnostics.extend(_validate_artifact_paths(evidence))
    diagnostics.extend(_validate_prerequisites_and_verdict(evidence))
    diagnostics.extend(_validate_bounded_refs(evidence, m029_selection, m030_selection))
    diagnostics.extend(_validate_readiness_counts(evidence, readiness_verify))
    diagnostics.extend(_validate_safety_and_raw_fields(evidence))
    diagnostics.extend(_validate_requirements(evidence))
    diagnostics.extend(_validate_report(evidence, report))
    diagnostics.extend(_validate_diagnostics(evidence, diagnostic_rows))
    return diagnostics


def _safe_summary_paths(args: argparse.Namespace) -> dict[str, str]:
    return {
        "evidence": args.evidence,
        "report": args.report,
        "diagnostics": args.diagnostics,
        "m029_selection": args.m029_selection,
        "m030_selection": args.m030_selection,
        "readiness_verify": args.readiness_verify,
    }


def _build_verify_summary(
    args: argparse.Namespace,
    evidence: Mapping[str, Any],
    diagnostics: Sequence[Mapping[str, Any]],
    m029_selection: Mapping[str, Any],
    m030_selection: Mapping[str, Any],
) -> dict[str, Any]:
    bounded_rows = (
        evidence.get("bounded_ref_reconciliation")
        if isinstance(evidence.get("bounded_ref_reconciliation"), list)
        else []
    )
    present_refs = sorted(
        str(row.get("bounded_ref_id"))
        # pyrefly: ignore [not-iterable]
        for row in bounded_rows  # ty:ignore[not-iterable]
        if isinstance(row, Mapping) and row.get("present_in_provisional_m029_selection") is True
    )
    missing_refs = sorted(
        str(row.get("bounded_ref_id"))
        # pyrefly: ignore [not-iterable]
        for row in bounded_rows  # ty:ignore[not-iterable]
        if isinstance(row, Mapping) and row.get("present_in_provisional_m029_selection") is False
    )
    readiness = (
        evidence.get("provisional_m029_readiness_counts")
        if isinstance(evidence.get("provisional_m029_readiness_counts"), Mapping)
        else {}
    )
    requirements = (
        evidence.get("requirement_coverage")
        if isinstance(evidence.get("requirement_coverage"), list)
        else []
    )
    return {
        "schema_version": VERIFY_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "failed" if diagnostics else "passed",
        "verdict": evidence.get("verdict"),
        "article_count": readiness.get("article_count"),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "ready_count": readiness.get("ready_count"),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "zero_chunk_count": readiness.get("zero_chunk_count"),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "readiness_status": readiness.get("status"),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "present_bounded_refs": present_refs,
        "missing_bounded_refs": missing_refs,
        "m029_selection_article_count": len(m029_selection.get("articles", []))
        if isinstance(m029_selection.get("articles"), list)
        else None,
        "m030_s01_bounded_ref_count": len(m030_selection.get("refs", []))
        if isinstance(m030_selection.get("refs"), list)
        else None,
        "requirement_ids": sorted(
            str(row.get("requirement_id"))
            # pyrefly: ignore [not-iterable]
            for row in requirements  # ty:ignore[not-iterable]
            if isinstance(row, Mapping) and row.get("requirement_id")
        ),
        "unsafe_flag_count": sum(
            1 for item in diagnostics if str(item.get("code", "")).endswith("UNSAFE_FLAG_TRUE")
        ),
        "diagnostic_count": len(diagnostics),
        "diagnostic_codes": sorted(Counter(str(item.get("code")) for item in diagnostics)),
        "source_artifact_paths": _safe_summary_paths(args),
        "metadata_only": True,
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_write_attempted": False,
        "diagnostics": list(diagnostics),
    }


def _write_json(path_value: str, payload: Mapping[str, Any], *, repo_root: Path) -> None:
    path = _repo_relative_path(
        path_value, repo_root=repo_root, label="write verify summary", require_exists=False
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def run(args: argparse.Namespace) -> int:
    repo_root = Path.cwd()
    evidence = _load_json_arg(args.evidence, repo_root=repo_root, label="evidence")
    report = _read_text_arg(args.report, repo_root=repo_root, label="report")
    diagnostic_rows = _read_jsonl_arg(args.diagnostics, repo_root=repo_root, label="diagnostics")
    m029_selection = _load_json_arg(
        args.m029_selection, repo_root=repo_root, label="M029 selection"
    )
    m030_selection = _load_json_arg(
        args.m030_selection, repo_root=repo_root, label="M030 selection"
    )
    readiness_verify = _load_json_arg(
        args.readiness_verify, repo_root=repo_root, label="readiness verify"
    )
    diagnostics = validate_remediation(
        evidence, report, diagnostic_rows, m029_selection, m030_selection, readiness_verify
    )
    verify_summary = _build_verify_summary(
        args, evidence, diagnostics, m029_selection, m030_selection
    )
    if args.write_verify_summary:
        _write_json(args.write_verify_summary, verify_summary, repo_root=repo_root)
    if diagnostics:
        sys.stderr.write("M029 validation remediation verification failed:\n")
        for item in diagnostics:
            sys.stderr.write(json.dumps(item, sort_keys=True) + "\n")
        return 1
    sys.stdout.write(json.dumps(verify_summary, sort_keys=True) + "\n")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True)
    parser.add_argument("--report", required=True)
    parser.add_argument("--diagnostics", required=True)
    parser.add_argument("--m029-selection", required=True)
    parser.add_argument("--m030-selection", required=True)
    parser.add_argument("--readiness-verify", required=True)
    parser.add_argument("--write-verify-summary")
    parser.add_argument(
        "--validate-only",
        action="store_true",
        help="Validate inputs without changing behavior; writes only when --write-verify-summary is supplied.",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv[1:] if argv and argv[0].endswith(".py") else argv)
    try:
        return run(args)
    except VerifierInputError as exc:
        sys.stderr.write(f"ERROR: {exc}\n")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
