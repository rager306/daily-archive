#!/usr/bin/env python3
"""Fail-closed verifier for the M029 post-validation remediation closure package.

The verifier is read-only except for optional ``--write-verify-summary``. It
accepts repo-relative metadata artifact paths, validates the S08 closure dossier
against S07/M029/M030 evidence, and refuses validation, production readiness,
graph/KG readiness, import readiness, or requirement-validation overclaims while
M030/S02-S06 and M030-derived M029 replan proof remain absent.
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
SLICE_ID = "S08"
TASK_ID = "T01"
SELECTION_ID = "m029-unified-corpus-v1"
SCHEMA_VERSION = "m029-post-validation-remediation-evidence.v1"
VERIFY_SCHEMA_VERSION = "m029-post-validation-remediation-verifier.v1"
S07_VERIFY_SCHEMA_VERSION = "m029-validation-remediation-verifier.v1"
EXPECTED_IN_SCOPE_REQUIREMENTS = ("R024", "R027", "R029", "R035", "R040", "R050")
EXPECTED_OUT_OF_SCOPE_REQUIREMENTS = (
    "R019",
    "R022",
    "R023",
    "R031",
    "R032",
    "R033",
    "R051",
    "R052",
)
BLOCKED_VERDICTS = {
    "blocked_pending_m030_completion_and_replan",
    "blocked_pending_m030_completion",
    "blocked_missing_m030_s02_s06_and_milestone_completion",
    "blocked_missing_m030_derived_m029_replan_proof",
    "blocked",
    "needs_remediation",
    "remediation_required",
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
    "metadata_only_boundary",
    "source_artifact_paths",
    "input_artifact_audit",
    "prerequisite_audit",
    "m030_derived_m029_replan_audit",
    "bounded_ref_counts",
    "bounded_ref_reconciliation",
    "provisional_m029_readiness_counts",
    "requirement_scope",
    "safety_flags",
    "forbidden_claims",
    "safe_closeout_wording",
    "remaining_remediation_scope",
    "diagnostic_codes",
    "diagnostic_count",
    "observability",
}
REQUIRED_FALSE_SAFETY_FLAGS = {
    "m029_validation_claimed",
    "production_readiness_claimed",
    "graph_or_kg_readiness_claimed",
    "import_readiness_claimed",
    "requirement_validation_claimed",
    "network_fetch_attempted",
    "source_write_attempted",
    "catalog_registration_modified",
    "source_loader_modified",
    "parser_or_chunker_modified",
    "graph_import_attempted",
    "graph_write_attempted",
    "ladybugdb_written",
    "production_import_attempted",
    "production_persistence_attempted",
    "raw_article_text_embedded",
    "raw_pdf_bytes_embedded",
    "binary_payload_embedded",
    "vectors_embedded",
    "secrets_embedded",
}
REQUIRED_FALSE_BOUNDARY_FLAGS = {
    "raw_article_text_included",
    "raw_pdf_bytes_included",
    "binary_payloads_included",
    "vectors_included",
    "secrets_included",
    "network_fetches_performed",
    "source_writes_performed",
    "graph_writes_performed",
    "requirement_record_changes_performed",
}
UNSAFE_TRUE_BOOLEAN_KEYS = (
    REQUIRED_FALSE_SAFETY_FLAGS
    | REQUIRED_FALSE_BOUNDARY_FLAGS
    | {
        "validated",
        "advanced",
        "validation_claim_allowed",
        "m030_completion_artifact_present",
        "m030_s06_roadmap_output_present",
        "m030_derived_m029_replan_proof_present",
        "production_import_allowed",
        "production_ladybugdb_write_allowed",
        "ready_for_graph_import",
        "graph_import_allowed",
        "trusted_kg_import_allowed",
        "kg_readiness_claimed",
        "graph_ready_claimed",
        "parser_ready_claimed",
        "chunk_ready_claimed",
        "source_acquisition_completed",
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
PATH_LIKE_KEYS = {
    "path",
    "evidence_path",
    "diagnostics_path",
    "expected_future_verify_summary_path",
    "m030_completion_artifact",
    "m030_s06_roadmap_output_path",
    "m029_roadmap",
    "m029_s07_summary",
    "m030_roadmap",
    "m030_s01_summary",
    "s07_remediation_evidence",
    "s07_remediation_report",
    "s07_remediation_diagnostics",
    "s07_remediation_verify_summary",
    "m029_readiness_verify_summary",
    "m029_selection",
    "m030_s01_selection",
    "m030_s01_intake_report",
}
PATH_LIST_KEYS = {"source_inputs", "source_artifact_paths"}
REQUIRED_DIAGNOSTIC_CODES = {
    "M029_POST_VALIDATION_MISSING_M030_COMPLETION",
    "M029_POST_VALIDATION_PENDING_M030_S02",
    "M029_POST_VALIDATION_PENDING_M030_S03",
    "M029_POST_VALIDATION_PENDING_M030_S04",
    "M029_POST_VALIDATION_PENDING_M030_S05",
    "M029_POST_VALIDATION_PENDING_M030_S06",
    "M029_POST_VALIDATION_MISSING_M030_S06_OUTPUT",
    "M029_POST_VALIDATION_MISSING_M029_REPLAN_PROOF",
    "M029_POST_VALIDATION_MISSING_BOUNDED_REF",
}
FORBIDDEN_POSITIVE_PHRASES = {
    "m029 validation passed",
    "m029 is validated",
    "m029 is production ready",
    "m029 is ready for graph import",
    "m029 is ready for kg import",
    "m029 import readiness is proven",
    "m030 completed s02-s06",
    "m030/s06 produced the implementation roadmap",
    "m029 was replanned from m030 outputs",
    "m029 was replanned from m030 output",
    "all four bounded refs are represented in the provisional m029 corpus",
    "all m030/s01 bounded refs are represented",
    "any m029 remediation requirement is validated",
    "r024 is validated",
    "r027 is validated",
    "r029 is validated",
    "r035 is validated",
    "r040 is validated",
    "r050 is validated",
    "ladybugdb was written",
    "production import was attempted",
}


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
    normalized = path_value.replace("\\", "/")
    path = Path(normalized)
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
            if isinstance(item, Mapping):
                item = item.get("path")
            if not isinstance(item, str) or not item:
                diagnostics.append(
                    _diagnostic(
                        "M029_POST_VALIDATION_UNSAFE_SOURCE_PATH",
                        f"source path must be a non-empty string: {item!r}",
                        json_path=path,
                    )
                )
                continue
            normalized = item.replace("\\", "/")
            if (
                "://" in item
                or Path(normalized).is_absolute()
                or any(part == ".." for part in Path(normalized).parts)
            ):
                diagnostics.append(
                    _diagnostic(
                        "M029_POST_VALIDATION_UNSAFE_SOURCE_PATH",
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
                "M029_POST_VALIDATION_MISSING_TOP_LEVEL_KEY",
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
                    "M029_POST_VALIDATION_IDENTITY_MISMATCH",
                    f"{key} must be {expected_value}",
                    json_path=json_path,
                )
            )
    if evidence.get("artifact_version") != 1:
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_SCHEMA_MISMATCH",
                "artifact_version must be 1",
                json_path="$.artifact_version",
            )
        )
    return diagnostics


def _validate_prerequisites(evidence: Mapping[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    prereq = evidence.get("prerequisite_audit")
    replan = evidence.get("m030_derived_m029_replan_audit")
    if not isinstance(prereq, Mapping):
        return [
            _diagnostic(
                "M029_POST_VALIDATION_MISSING_PREREQUISITE_AUDIT",
                "prerequisite_audit must be an object",
                json_path="$.prerequisite_audit",
            )
        ]
    if not isinstance(replan, Mapping):
        return [
            _diagnostic(
                "M029_POST_VALIDATION_MISSING_REPLAN_AUDIT",
                "m030_derived_m029_replan_audit must be an object",
                json_path="$.m030_derived_m029_replan_audit",
            )
        ]

    missing_blockers = False
    if prereq.get("m030_completion_artifact_present") is not False:
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_PREREQUISITE_OVERCLAIM",
                "M030 completion artifact must remain absent for this blocked dossier",
                json_path="$.prerequisite_audit.m030_completion_artifact_present",
            )
        )
    else:
        missing_blockers = True

    slices = prereq.get("slices")
    if not isinstance(slices, list):
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_MISSING_PREREQUISITE_AUDIT",
                "prerequisite_audit.slices must be a list",
                json_path="$.prerequisite_audit.slices",
            )
        )
    else:
        by_slice = {row.get("slice_id"): row for row in slices if isinstance(row, Mapping)}
        for slice_id in ("S02", "S03", "S04", "S05", "S06"):
            row = by_slice.get(slice_id)
            if not isinstance(row, Mapping):
                diagnostics.append(
                    _diagnostic(
                        f"M029_POST_VALIDATION_PENDING_M030_{slice_id}",
                        f"M030/{slice_id} prerequisite row is missing",
                        json_path="$.prerequisite_audit.slices",
                    )
                )
                missing_blockers = True
                continue
            if row.get("status") != "pending" or row.get("evidence_present") is not False:
                diagnostics.append(
                    _diagnostic(
                        "M029_POST_VALIDATION_PREREQUISITE_OVERCLAIM",
                        f"M030/{slice_id} must remain pending with absent evidence",
                        json_path=f"$.prerequisite_audit.slices[{slices.index(row)}]",
                    )
                )
            else:
                missing_blockers = True

    if replan.get("m030_s06_roadmap_output_present") is not False:
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_REPLAN_OVERCLAIM",
                "M030/S06 roadmap output must remain absent",
                json_path="$.m030_derived_m029_replan_audit.m030_s06_roadmap_output_present",
            )
        )
    else:
        missing_blockers = True
    if any(
        isinstance(row, Mapping) and row.get("present") is True
        for row in replan.get("candidate_replan_artifacts", [])
    ):
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_REPLAN_OVERCLAIM",
                "M029 replan artifacts must not be marked present without post-M030 proof",
                json_path="$.m030_derived_m029_replan_audit.candidate_replan_artifacts",
            )
        )
    elif replan.get("candidate_replan_artifacts"):
        missing_blockers = True

    if missing_blockers and str(evidence.get("verdict")) not in BLOCKED_VERDICTS:
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_FALSE_PASS_VERDICT",
                "verdict must remain blocked while M030/M029 replan prerequisites are missing",
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
    return {
        str(row["ref_id"]): row
        for row in refs
        if isinstance(row, Mapping) and isinstance(row.get("ref_id"), str)
    }


def _validate_bounded_refs(
    evidence: Mapping[str, Any],
    m029_selection: Mapping[str, Any],
    m030_selection: Mapping[str, Any],
    s07_evidence: Mapping[str, Any],
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    rows = evidence.get("bounded_ref_reconciliation")
    if not isinstance(rows, list):
        return [
            _diagnostic(
                "M029_POST_VALIDATION_BOUNDED_REF_MISMATCH",
                "bounded_ref_reconciliation must be a list",
                json_path="$.bounded_ref_reconciliation",
            )
        ]
    row_by_ref = {row.get("bounded_ref_id"): row for row in rows if isinstance(row, Mapping)}
    refs = _m030_refs(m030_selection)
    identities = _selection_identity_set(m029_selection, rows_key="articles")
    if set(row_by_ref) != set(refs):
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_BOUNDED_REF_MISMATCH",
                # pyrefly: ignore [bad-specialization]
                f"bounded refs must match M030/S01 refs; missing={sorted(set(refs) - set(row_by_ref))}, extra={sorted(set(row_by_ref) - set(refs))}",
                json_path="$.bounded_ref_reconciliation",
            )
        )
    missing_count = 0
    represented_count = 0
    for index, row in enumerate(rows):
        if not isinstance(row, Mapping):
            diagnostics.append(
                _diagnostic(
                    "M029_POST_VALIDATION_BOUNDED_REF_MISMATCH",
                    "bounded ref row must be an object",
                    json_path=f"$.bounded_ref_reconciliation[{index}]",
                )
            )
            continue
        ref = refs.get(str(row.get("bounded_ref_id")))
        if not ref:
            continue
        identity = ref.get("normalized_identity")
        if row.get("normalized_identity") != identity:
            diagnostics.append(
                _diagnostic(
                    "M029_POST_VALIDATION_BOUNDED_REF_MISMATCH",
                    "bounded ref normalized_identity must match M030/S01 selection",
                    json_path=f"$.bounded_ref_reconciliation[{index}].normalized_identity",
                )
            )
        expected_present = isinstance(identity, str) and identity in identities
        if row.get("present_in_provisional_m029_selection") is not expected_present:
            diagnostics.append(
                _diagnostic(
                    "M029_POST_VALIDATION_BOUNDED_REF_MISMATCH",
                    "present_in_provisional_m029_selection does not match M029 selection",
                    json_path=f"$.bounded_ref_reconciliation[{index}].present_in_provisional_m029_selection",
                )
            )
        if expected_present:
            represented_count += 1
            if row.get("reconciliation_status") != "represented_in_provisional_m029_corpus":
                diagnostics.append(
                    _diagnostic(
                        "M029_POST_VALIDATION_BOUNDED_REF_MISMATCH",
                        "represented bounded ref has wrong reconciliation_status",
                        json_path=f"$.bounded_ref_reconciliation[{index}].reconciliation_status",
                    )
                )
        else:
            missing_count += 1
            if row.get("reconciliation_status") != "missing_from_provisional_m029_corpus":
                diagnostics.append(
                    _diagnostic(
                        "M029_POST_VALIDATION_BOUNDED_REF_MISMATCH",
                        "missing bounded ref has wrong reconciliation_status",
                        json_path=f"$.bounded_ref_reconciliation[{index}].reconciliation_status",
                    )
                )
            action = str(row.get("safe_next_action", ""))
            if "redo" not in action and "descope" not in action:
                diagnostics.append(
                    _diagnostic(
                        "M029_POST_VALIDATION_MISSING_BOUNDED_REF",
                        "missing bounded ref must remain redo or explicit-descope scope",
                        json_path=f"$.bounded_ref_reconciliation[{index}].safe_next_action",
                    )
                )
    counts = evidence.get("bounded_ref_counts")
    expected_counts = {
        "m030_s01_bounded_ref_count": len(refs),
        "represented_in_provisional_m029_count": represented_count,
        "missing_from_provisional_m029_count": missing_count,
    }
    if not isinstance(counts, Mapping):
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_BOUNDED_REF_COUNT_DRIFT",
                "bounded_ref_counts must be an object",
                json_path="$.bounded_ref_counts",
            )
        )
    else:
        for key, expected in expected_counts.items():
            if counts.get(key) != expected:
                diagnostics.append(
                    _diagnostic(
                        "M029_POST_VALIDATION_BOUNDED_REF_COUNT_DRIFT",
                        f"{key} must be {expected}",
                        json_path=f"$.bounded_ref_counts.{key}",
                    )
                )
            s07_counts = (
                s07_evidence.get("bounded_ref_counts")
                if isinstance(s07_evidence.get("bounded_ref_counts"), Mapping)
                else {}
            )
            if s07_counts and counts.get(key) != s07_counts.get(key):
                diagnostics.append(
                    _diagnostic(
                        "M029_POST_VALIDATION_BOUNDED_REF_COUNT_DRIFT",
                        f"{key} must match S07 remediation evidence",
                        json_path=f"$.bounded_ref_counts.{key}",
                    )
                )
    if missing_count and "M029_POST_VALIDATION_MISSING_BOUNDED_REF" not in set(
        evidence.get("diagnostic_codes", [])
    ):
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_DIAGNOSTIC_DRIFT",
                "missing bounded refs require M029_POST_VALIDATION_MISSING_BOUNDED_REF diagnostic code",
                json_path="$.diagnostic_codes",
            )
        )
    return diagnostics


def _validate_readiness_counts(
    evidence: Mapping[str, Any], readiness_verify: Mapping[str, Any], s07_summary: Mapping[str, Any]
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    counts = evidence.get("provisional_m029_readiness_counts")
    if not isinstance(counts, Mapping):
        return [
            _diagnostic(
                "M029_POST_VALIDATION_READINESS_COUNT_DRIFT",
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
        if counts.get(key) != readiness_verify.get(key):
            diagnostics.append(
                _diagnostic(
                    "M029_POST_VALIDATION_READINESS_COUNT_DRIFT",
                    f"{key} must match M029 readiness summary",
                    json_path=f"$.provisional_m029_readiness_counts.{key}",
                )
            )
    if s07_summary:
        for key in ("article_count", "ready_count", "zero_chunk_count", "unsafe_flag_count"):
            if counts.get(key) != s07_summary.get(key):
                diagnostics.append(
                    _diagnostic(
                        "M029_POST_VALIDATION_READINESS_COUNT_DRIFT",
                        f"{key} must match S07 verify summary",
                        json_path=f"$.provisional_m029_readiness_counts.{key}",
                    )
                )
    if counts.get("status") != "passed" or counts.get("unsafe_flag_count") != 0:
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_READINESS_COUNT_DRIFT",
                "readiness evidence must be local passed evidence with zero unsafe flags",
                json_path="$.provisional_m029_readiness_counts",
            )
        )
    return diagnostics


def _validate_safety_and_payloads(evidence: Mapping[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    flags = evidence.get("safety_flags")
    boundary = evidence.get("metadata_only_boundary")
    if not isinstance(flags, Mapping):
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_UNSAFE_FLAG_TRUE",
                "safety_flags must be an object",
                json_path="$.safety_flags",
            )
        )
    else:
        for key in sorted(REQUIRED_FALSE_SAFETY_FLAGS):
            if flags.get(key) is not False:
                diagnostics.append(
                    _diagnostic(
                        "M029_POST_VALIDATION_UNSAFE_FLAG_TRUE",
                        f"safety flag must be false: {key}",
                        json_path=f"$.safety_flags.{key}",
                    )
                )
    if not isinstance(boundary, Mapping):
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_UNSAFE_FLAG_TRUE",
                "metadata_only_boundary must be an object",
                json_path="$.metadata_only_boundary",
            )
        )
    else:
        if boundary.get("relative_paths_only") is not True:
            diagnostics.append(
                _diagnostic(
                    "M029_POST_VALIDATION_UNSAFE_SOURCE_PATH",
                    "metadata_only_boundary.relative_paths_only must be true",
                    json_path="$.metadata_only_boundary.relative_paths_only",
                )
            )
        for key in sorted(REQUIRED_FALSE_BOUNDARY_FLAGS):
            if boundary.get(key) is not False:
                diagnostics.append(
                    _diagnostic(
                        "M029_POST_VALIDATION_RAW_PAYLOAD_FIELD",
                        f"metadata-only boundary flag must be false: {key}",
                        json_path=f"$.metadata_only_boundary.{key}",
                    )
                )
    for path, value, _ancestors in _walk(evidence):
        field = _field_name(path)
        if isinstance(value, bool) and value is True and field in UNSAFE_TRUE_BOOLEAN_KEYS:
            diagnostics.append(
                _diagnostic(
                    "M029_POST_VALIDATION_UNSAFE_FLAG_TRUE",
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
                        "M029_POST_VALIDATION_RAW_PAYLOAD_FIELD",
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
                        "M029_POST_VALIDATION_RAW_PAYLOAD_FIELD",
                        "string contains raw payload/base64/secret marker",
                        json_path=path,
                    )
                )
    return diagnostics


def _validate_requirements(evidence: Mapping[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    scope = evidence.get("requirement_scope")
    if not isinstance(scope, Mapping):
        return [
            _diagnostic(
                "M029_POST_VALIDATION_REQUIREMENT_SCOPE_DRIFT",
                "requirement_scope must be an object",
                json_path="$.requirement_scope",
            )
        ]
    in_scope = scope.get("in_scope_m029_remediation_requirements")
    out_scope = scope.get("out_of_scope_project_requirements")
    if not isinstance(in_scope, list):
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_REQUIREMENT_SCOPE_DRIFT",
                "in-scope requirement rows must be a list",
                json_path="$.requirement_scope.in_scope_m029_remediation_requirements",
            )
        )
        in_scope = []
    if not isinstance(out_scope, list):
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_REQUIREMENT_SCOPE_DRIFT",
                "out-of-scope requirement rows must be a list",
                json_path="$.requirement_scope.out_of_scope_project_requirements",
            )
        )
        out_scope = []
    in_ids = {row.get("requirement_id") for row in in_scope if isinstance(row, Mapping)}
    out_ids = {row.get("requirement_id") for row in out_scope if isinstance(row, Mapping)}
    if in_ids != set(EXPECTED_IN_SCOPE_REQUIREMENTS):
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_REQUIREMENT_SCOPE_DRIFT",
                "in-scope requirements must be exactly "
                + ", ".join(EXPECTED_IN_SCOPE_REQUIREMENTS),
                json_path="$.requirement_scope.in_scope_m029_remediation_requirements",
            )
        )
    if out_ids != set(EXPECTED_OUT_OF_SCOPE_REQUIREMENTS):
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_REQUIREMENT_SCOPE_DRIFT",
                "out-of-scope requirements must be exactly "
                + ", ".join(EXPECTED_OUT_OF_SCOPE_REQUIREMENTS),
                json_path="$.requirement_scope.out_of_scope_project_requirements",
            )
        )
    for index, row in enumerate(in_scope):
        if not isinstance(row, Mapping):
            continue
        rid = row.get("requirement_id")
        if row.get("coverage_status") != "advanced_not_validated":
            diagnostics.append(
                _diagnostic(
                    "M029_POST_VALIDATION_REQUIREMENT_STATUS_OVERCLAIM",
                    f"{rid} coverage_status must remain advanced_not_validated",
                    json_path=f"$.requirement_scope.in_scope_m029_remediation_requirements[{index}].coverage_status",
                )
            )
        if row.get("validated") is not False or row.get("validation_claim_allowed") is not False:
            diagnostics.append(
                _diagnostic(
                    "M029_POST_VALIDATION_REQUIREMENT_STATUS_OVERCLAIM",
                    f"{rid} must not claim validation",
                    json_path=f"$.requirement_scope.in_scope_m029_remediation_requirements[{index}]",
                )
            )
    for index, row in enumerate(out_scope):
        if not isinstance(row, Mapping):
            continue
        rid = row.get("requirement_id")
        if row.get("scope_status") != "out_of_scope_for_m029_post_validation_remediation":
            diagnostics.append(
                _diagnostic(
                    "M029_POST_VALIDATION_REQUIREMENT_SCOPE_DRIFT",
                    f"{rid} must remain out of scope",
                    json_path=f"$.requirement_scope.out_of_scope_project_requirements[{index}].scope_status",
                )
            )
        if row.get("advanced") is not False or row.get("validated") is not False:
            diagnostics.append(
                _diagnostic(
                    "M029_POST_VALIDATION_REQUIREMENT_STATUS_OVERCLAIM",
                    f"{rid} must not be advanced or validated",
                    json_path=f"$.requirement_scope.out_of_scope_project_requirements[{index}]",
                )
            )
    if (
        scope.get("requirement_records_modified") is not False
        or scope.get("validated_requirement_count") != 0
    ):
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_REQUIREMENT_STATUS_OVERCLAIM",
                "requirement records must remain unmodified and validated count must be zero",
                json_path="$.requirement_scope",
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
        "# M029 Post-Validation Remediation Closure Report",
        str(evidence.get("verdict")),
        "## Prerequisite Audit",
        "## M030-Derived M029 Replan Audit",
        "## Bounded-Ref Reconciliation",
        "## Provisional M029 Readiness Context",
        "## In-Scope M029 Requirement Coverage",
        "## Out-of-Scope Project Requirements",
        "## Safety Flags",
        "## Forbidden Claims",
        "## Remaining Remediation Scope",
        "## Failure Modes (Q5)",
        "## Load Profile (Q6)",
        "## Negative Tests (Q7)",
        "## Observability Impact",
    ]
    for marker in required_markers:
        if marker and marker not in report:
            diagnostics.append(
                _diagnostic(
                    "M029_POST_VALIDATION_REPORT_SYNC_DRIFT",
                    f"report missing marker: {marker}",
                    path="report",
                )
            )
    counts = (
        evidence.get("provisional_m029_readiness_counts")
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
        value = counts.get(key)  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        if isinstance(value, (str, int)) and str(value) not in report:
            diagnostics.append(
                _diagnostic(
                    "M029_POST_VALIDATION_REPORT_SYNC_DRIFT",
                    f"report missing readiness value: {value}",
                    path="report",
                )
            )
    positive_text = _report_without_forbidden_section(report).lower()
    for phrase in sorted(FORBIDDEN_POSITIVE_PHRASES):
        if phrase in positive_text:
            diagnostics.append(
                _diagnostic(
                    "M029_POST_VALIDATION_FORBIDDEN_POSITIVE_CLAIM",
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
                    "M029_POST_VALIDATION_RAW_PAYLOAD_FIELD",
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
                "M029_POST_VALIDATION_DIAGNOSTIC_DRIFT",
                "diagnostic_count does not match JSONL rows",
                json_path="$.diagnostic_count",
            )
        )
    codes = [str(row.get("code") or row.get("diagnostic_code") or "") for row in diagnostic_rows]
    if set(evidence.get("diagnostic_codes", [])) != set(codes):
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_DIAGNOSTIC_DRIFT",
                "diagnostic_codes do not match JSONL rows",
                json_path="$.diagnostic_codes",
            )
        )
    missing_required = sorted(REQUIRED_DIAGNOSTIC_CODES - set(codes))
    if missing_required:
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_DIAGNOSTIC_DRIFT",
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
                        "M029_POST_VALIDATION_DIAGNOSTIC_DRIFT",
                        f"diagnostic row missing {key}",
                        json_path=row_path,
                    )
                )
        if row.get("severity") != "blocker":
            diagnostics.append(
                _diagnostic(
                    "M029_POST_VALIDATION_DIAGNOSTIC_DRIFT",
                    "diagnostic severity must remain blocker",
                    json_path=row_path,
                )
            )
    return diagnostics


def _validate_optional_evidence_paths(
    optional_paths: Sequence[str], *, repo_root: Path
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    for index, path_value in enumerate(optional_paths):
        try:
            _repo_relative_path(
                path_value, repo_root=repo_root, label=f"optional evidence path {index + 1}"
            )
        except VerifierInputError as exc:
            diagnostics.append(
                _diagnostic(
                    "M029_POST_VALIDATION_OPTIONAL_EVIDENCE_PATH_INVALID",
                    str(exc),
                    json_path=f"optional_evidence_paths[{index}]",
                    path=path_value,
                )
            )
    return diagnostics


def validate_remediation(
    evidence: Mapping[str, Any],
    report: str,
    diagnostic_rows: Sequence[Mapping[str, Any]],
    s07_evidence: Mapping[str, Any],
    s07_summary: Mapping[str, Any],
    m029_selection: Mapping[str, Any],
    readiness_verify: Mapping[str, Any],
    m030_selection: Mapping[str, Any],
) -> list[dict[str, Any]]:
    """Return fail-closed diagnostics for the S08 post-validation package."""

    diagnostics: list[dict[str, Any]] = []
    diagnostics.extend(_validate_shape(evidence))
    diagnostics.extend(_validate_artifact_paths(evidence))
    diagnostics.extend(_validate_prerequisites(evidence))
    diagnostics.extend(
        _validate_bounded_refs(evidence, m029_selection, m030_selection, s07_evidence)
    )
    diagnostics.extend(_validate_readiness_counts(evidence, readiness_verify, s07_summary))
    diagnostics.extend(_validate_safety_and_payloads(evidence))
    diagnostics.extend(_validate_requirements(evidence))
    diagnostics.extend(_validate_report(evidence, report))
    diagnostics.extend(_validate_diagnostics(evidence, diagnostic_rows))
    if s07_summary and s07_summary.get("schema_version") != S07_VERIFY_SCHEMA_VERSION:
        diagnostics.append(
            _diagnostic(
                "M029_POST_VALIDATION_S07_SUMMARY_DRIFT",
                "S07 verify summary schema mismatch",
                json_path="s07_summary.schema_version",
            )
        )
    return diagnostics


def _safe_summary_paths(args: argparse.Namespace) -> dict[str, Any]:
    return {
        "evidence": args.evidence,
        "report": args.report,
        "diagnostics": args.diagnostics,
        "s07_evidence": args.s07_evidence,
        "s07_verify_summary": args.s07_verify_summary,
        "m029_selection": args.m029_selection,
        "m029_readiness_summary": args.m029_readiness_summary,
        "m030_requested_ref_selection": args.m030_requested_ref_selection,
        "optional_prerequisite_evidence": args.prerequisite_evidence,
        "optional_replan_evidence": args.replan_evidence,
    }


def _build_verify_summary(
    args: argparse.Namespace,
    evidence: Mapping[str, Any],
    diagnostics: Sequence[Mapping[str, Any]],
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
    scope = (
        evidence.get("requirement_scope")
        if isinstance(evidence.get("requirement_scope"), Mapping)
        else {}
    )
    in_scope = (
        scope.get("in_scope_m029_remediation_requirements")  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        if isinstance(scope.get("in_scope_m029_remediation_requirements"), list)  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        else []
    )
    out_scope = (
        scope.get("out_of_scope_project_requirements")  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        if isinstance(scope.get("out_of_scope_project_requirements"), list)  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        else []
    )
    return {
        "schema_version": VERIFY_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "failed" if diagnostics else "passed",
        "verdict": evidence.get("verdict"),
        "blocked_verdict": str(evidence.get("verdict")) in BLOCKED_VERDICTS,
        "article_count": readiness.get("article_count"),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "ready_count": readiness.get("ready_count"),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "zero_chunk_count": readiness.get("zero_chunk_count"),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "readiness_status": readiness.get("status"),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "m030_s01_bounded_ref_count": len(m030_selection.get("refs", []))
        if isinstance(m030_selection.get("refs"), list)
        else None,
        "present_bounded_refs": present_refs,
        "missing_bounded_refs": missing_refs,
        "represented_bounded_ref_count": len(present_refs),
        "missing_bounded_ref_count": len(missing_refs),
        "in_scope_requirement_ids": sorted(
            str(row.get("requirement_id"))
            for row in in_scope  # ty:ignore[not-iterable]
            if isinstance(row, Mapping) and row.get("requirement_id")
        ),
        "out_of_scope_requirement_ids": sorted(
            str(row.get("requirement_id"))
            for row in out_scope  # ty:ignore[not-iterable]
            if isinstance(row, Mapping) and row.get("requirement_id")
        ),
        "validated_requirement_count": scope.get("validated_requirement_count"),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
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
    s07_evidence = _load_json_arg(args.s07_evidence, repo_root=repo_root, label="S07 evidence")
    s07_summary = _load_json_arg(
        args.s07_verify_summary, repo_root=repo_root, label="S07 verify summary"
    )
    m029_selection = _load_json_arg(
        args.m029_selection, repo_root=repo_root, label="M029 selection"
    )
    readiness_verify = _load_json_arg(
        args.m029_readiness_summary, repo_root=repo_root, label="M029 readiness summary"
    )
    m030_selection = _load_json_arg(
        args.m030_requested_ref_selection, repo_root=repo_root, label="M030 requested-ref selection"
    )
    diagnostics = validate_remediation(
        evidence,
        report,
        diagnostic_rows,
        s07_evidence,
        s07_summary,
        m029_selection,
        readiness_verify,
        m030_selection,
    )
    diagnostics.extend(
        _validate_optional_evidence_paths(
            [*args.prerequisite_evidence, *args.replan_evidence], repo_root=repo_root
        )
    )
    verify_summary = _build_verify_summary(args, evidence, diagnostics, m030_selection)
    if args.write_verify_summary:
        _write_json(args.write_verify_summary, verify_summary, repo_root=repo_root)
    if diagnostics:
        sys.stderr.write("M029 post-validation remediation verification failed:\n")
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
    parser.add_argument("--s07-evidence", required=True)
    parser.add_argument("--s07-verify-summary", required=True)
    parser.add_argument("--m029-selection", required=True)
    parser.add_argument("--m029-readiness-summary", required=True)
    parser.add_argument("--m030-requested-ref-selection", required=True)
    parser.add_argument(
        "--prerequisite-evidence",
        action="append",
        default=[],
        help="Optional repo-relative prerequisite evidence path; if supplied it must exist under repo root.",
    )
    parser.add_argument(
        "--replan-evidence",
        action="append",
        default=[],
        help="Optional repo-relative replan proof path; if supplied it must exist under repo root.",
    )
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
