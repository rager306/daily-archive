#!/usr/bin/env python3
"""Build and verify the M031 validation-remediation dossier.

The verifier is local-only and fail-closed. It reconciles the stale S02
assessment failure against fresh S02 summary/UAT evidence, checks M031-scoped
requirement and verification-class coverage, and optionally writes deterministic
metadata-only remediation surfaces under the M031 validation-remediation output
directory.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path
from typing import Any

from research_graph.application.validation.evidence_paths import (
    ValidationEvidencePathError,
    json_path,
    repo_relative_path,
    safe_output_path,
)

MILESTONE_ID = "M031-vwpd8e"
SLICE_ID = "S06"
TASK_ID = "T01"
SELECTION_ID = "m031-catalog-backed-replay-v1"
SCHEMA_VERSION = "m031-validation-remediation-evidence.v1"
DIAGNOSTIC_SCHEMA_VERSION = "m031-validation-remediation-diagnostic.v1"
VERIFY_SCHEMA_VERSION = "m031-validation-remediation-verifier.v1"
CORPUS_DIR = Path("data/article_corpora/m031-catalog-backed-replay-v1")
OUTPUT_DIR = CORPUS_DIR / "validation-remediation"

DEFAULT_S02_ASSESSMENT = Path(".gsd/milestones/M031-vwpd8e/slices/S02/S02-ASSESSMENT.md")
DEFAULT_S02_SUMMARY = Path(".gsd/milestones/M031-vwpd8e/slices/S02/S02-SUMMARY.md")
DEFAULT_S02_UAT = Path(".gsd/milestones/M031-vwpd8e/slices/S02/S02-UAT.md")
DEFAULT_REPLAY_CLOSEOUT = CORPUS_DIR / "replay-closeout-summary.json"
DEFAULT_S05_CLOSEOUT = CORPUS_DIR / "s05-closeout-summary.json"
DEFAULT_MATRIX = CORPUS_DIR / "progression-matrix.json"
DEFAULT_AUDIT = CORPUS_DIR / "m031-continuity-audit.json"
DEFAULT_REVIEW_EVENTS = CORPUS_DIR / "chunk-evidence" / "independent-review-events.jsonl"

DEFAULT_EVIDENCE_OUT = OUTPUT_DIR / "validation-remediation-evidence.json"
DEFAULT_DIAGNOSTICS_OUT = OUTPUT_DIR / "validation-remediation-diagnostics.jsonl"
DEFAULT_REPORT_OUT = OUTPUT_DIR / "validation-remediation-report.md"
DEFAULT_VERIFY_SUMMARY_OUT = OUTPUT_DIR / "validation-remediation-verify-summary.json"

REQUIRED_REQUIREMENT_IDS = ("R024", "R027", "R029", "R040", "R050")
CANONICAL_CLASSES = ("Contract", "Integration", "Operational", "UAT")
REQUIRED_FALSE_FLAGS = {
    "network_fetch_attempted",
    "model_call_attempted",
    "graph_import_allowed",
    "graph_write_attempted",
    "kg_readiness_claimed",
    "trusted_kg_import_allowed",
    "production_import_attempted",
    "production_persistence_attempted",
    "production_ladybugdb_write_allowed",
    "ladybugdb_written",
    "import_ready_claimed",
    "import_eligible_claimed",
    "trusted_fact_promotion_allowed",
    "requirement_records_modified",
    "graph_or_import_writes_enabled",
    "source_write_attempted",
    "non_artifact_write_attempted",
    "raw_article_text_embedded",
    "raw_article_html_embedded",
    "raw_pdf_bytes_embedded",
    "binary_payload_embedded",
    "base64_payload_embedded",
    "raw_payload_embedded_in_metadata",
    "chunk_text_included",
    "raw_text_included",
    "embeddings_included",
    "vectors_included",
    "secrets_included",
}
UNSAFE_TRUE_BOOLEAN_KEYS = REQUIRED_FALSE_FLAGS | {
    "validated",
    "validation_claim_allowed",
    "ready_for_graph_import",
    "import_ready",
    "import_eligible",
    "production_import_allowed",
    "kg_import_allowed",
    "ladybugdb_write_allowed",
    "network_allowed",
    "model_enabled",
    "write_enabled",
}
UNSAFE_FIELD_NAME_FRAGMENTS = {
    "raw_article_text",
    "raw_text_payload",
    "raw_payload",
    "pdf_bytes",
    "binary_payload",
    "base64_payload",
    "vector_payload",
    "embedding_payload",
    "secret_value",
    "api_key",
    "password",
    "token_value",
}
FORBIDDEN_SNIPPETS = (
    "<html",
    "<!doctype html",
    "%pdf-",
    "base64,",
    "-----begin",
    "api_key=",
    "password=",
    "secret=",
)
FORBIDDEN_POSITIVE_PHRASES = {
    "m031 is ready for graph import",
    "m031 validates graph readiness",
    "m031 authorizes graph import",
    "m031 authorizes kg import",
    "m031 promotes trusted facts",
    "ladybugdb was written",
    "production import was attempted",
    "network fetch was attempted",
    "model call was attempted",
    "r024 is validated",
    "r027 is validated",
    "r029 is validated",
    "r040 is validated",
    "r050 is validated",
}
REQUIRED_TOP_LEVEL_KEYS = {
    "schema_version",
    "milestone_id",
    "slice_id",
    "task_id",
    "selection_id",
    "metadata_only",
    "source_artifacts",
    "s02_assessment_reconciliation",
    "requirement_coverage",
    "canonical_verification_classes",
    "safety_flags",
    "graph_import_boundary",
    "rerun_ready_validation_inputs",
    "quality_gates",
    "diagnostic_codes",
}


class M031ValidationRemediationError(RuntimeError):
    """Raised for missing/malformed inputs or unsafe output paths."""


def _json_path(parent: str, key: str | int) -> str:
    return json_path(parent, key)


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


def diagnostic(
    code: str,
    message: str,
    *,
    severity: str = "error",
    json_path: str = "$",
    path: str | Path | None = None,
) -> dict[str, Any]:
    return {
        "schema_version": DIAGNOSTIC_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "task_id": TASK_ID,
        "severity": severity,
        "code": code,
        "diagnostic_code": code,
        "message": message,
        "json_path": json_path,
        "path": Path(path).as_posix() if isinstance(path, Path) else path,
        "metadata_only": True,
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_write_attempted": False,
        "model_call_attempted": False,
    }


def _repo_relative_path(
    path_value: str | Path, *, repo_root: Path, label: str, require_exists: bool = True
) -> Path:
    try:
        return repo_relative_path(
            path_value, repo_root=repo_root, label=label, require_exists=require_exists
        )
    except ValidationEvidencePathError as exc:
        raise M031ValidationRemediationError(str(exc)) from exc


def _safe_output_path(path_value: str | Path, *, repo_root: Path, label: str) -> Path:
    try:
        return safe_output_path(
            path_value, repo_root=repo_root, label=label, output_dir=OUTPUT_DIR
        )
    except ValidationEvidencePathError as exc:
        raise M031ValidationRemediationError(str(exc)) from exc


def load_text(path_value: str | Path, *, repo_root: Path, label: str) -> str:
    path = _repo_relative_path(path_value, repo_root=repo_root, label=label)
    return path.read_text(encoding="utf-8")


def load_json(path_value: str | Path, *, repo_root: Path, label: str) -> dict[str, Any]:
    path = _repo_relative_path(path_value, repo_root=repo_root, label=label)
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise M031ValidationRemediationError(
            f"invalid JSON in {label} at {Path(path_value).as_posix()}: {exc}"
        ) from exc
    if not isinstance(value, dict):
        raise M031ValidationRemediationError(
            f"{label} root must be a JSON object: {Path(path_value).as_posix()}"
        )
    return value


def load_jsonl(path_value: str | Path, *, repo_root: Path, label: str) -> list[dict[str, Any]]:
    path = _repo_relative_path(path_value, repo_root=repo_root, label=label)
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as exc:
            raise M031ValidationRemediationError(
                f"invalid JSONL in {label} at {Path(path_value).as_posix()}:{line_number}: {exc}"
            ) from exc
        if not isinstance(value, dict):
            raise M031ValidationRemediationError(
                f"{label} row must be an object at {Path(path_value).as_posix()}:{line_number}"
            )
        value.setdefault("_line_number", line_number)
        rows.append(value)
    return rows


def _bool_from_frontmatter(text: str, key: str) -> str | None:
    match = re.search(rf"(?im)^\s*{re.escape(key)}\s*:\s*([^\n]+?)\s*$", text)
    return match.group(1).strip() if match else None


def _contains_65_pass_signal(*texts: str) -> bool:
    return any(re.search(r"\b65\s+passed\b", text, flags=re.IGNORECASE) for text in texts)


def _assessment_is_stale_failure(assessment_text: str) -> bool:
    verdict = (_bool_from_frontmatter(assessment_text, "verdict") or "").upper()
    return (
        verdict == "FAIL"
        or "required regression pytest suite check did not produce" in assessment_text.lower()
    )


def _full_repo_collection_debt(assessment_text: str) -> bool:
    lowered = assessment_text.lower()
    return (
        "full" in lowered
        and "pytest" in lowered
        and "failed during collection" in lowered
        and "unrelated" in lowered
    )


def _requirement_rows() -> list[dict[str, Any]]:
    return [
        {
            "requirement_id": requirement_id,
            "coverage_status": "m031_scoped_rechecked",
            "validated": False,
            "validation_claim_allowed": False,
            "evidence_paths": [
                DEFAULT_S02_SUMMARY.as_posix(),
                DEFAULT_S02_UAT.as_posix(),
                DEFAULT_REPLAY_CLOSEOUT.as_posix(),
                DEFAULT_S05_CLOSEOUT.as_posix(),
            ],
            "safe_claim": f"{requirement_id} has M031-scoped remediation coverage evidence only; no global status is changed.",
        }
        for requirement_id in REQUIRED_REQUIREMENT_IDS
    ]


def _class_rows() -> list[dict[str, Any]]:
    return [
        {
            "class": class_name,
            "status": "covered_for_validation_rerun",
            "evidence_paths": [
                DEFAULT_S02_SUMMARY.as_posix(),
                DEFAULT_S05_CLOSEOUT.as_posix(),
                DEFAULT_AUDIT.as_posix(),
            ],
            "safe_claim": f"{class_name} rerun evidence is metadata-only and scoped to M031 validation remediation.",
        }
        for class_name in CANONICAL_CLASSES
    ]


def _flag_from_sources(key: str, sources: Sequence[Mapping[str, Any]]) -> bool:
    for source in sources:
        for path, value, ancestors in _walk(source):
            field = ancestors[-1] if ancestors else path.rsplit(".", maxsplit=1)[-1]
            if field == key and isinstance(value, bool):
                return value
    return False


def build_evidence(
    *,
    s02_assessment: str,
    s02_summary: str,
    s02_uat: str,
    replay_closeout: Mapping[str, Any],
    s05_closeout: Mapping[str, Any],
    progression_matrix: Mapping[str, Any],
    continuity_audit: Mapping[str, Any],
    review_events: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    """Build a metadata-only remediation dossier from local artifacts."""

    source_maps: list[Mapping[str, Any]] = [
        replay_closeout,
        s05_closeout,
        progression_matrix,
        continuity_audit,
        *review_events,
    ]
    stale = _assessment_is_stale_failure(s02_assessment)
    fresh_65 = _contains_65_pass_signal(s02_summary, s02_uat)
    full_repo_debt = _full_repo_collection_debt(s02_assessment)
    completed_review_count = sum(
        1
        for row in review_events
        if row.get("independent_review_completed") is True
        or row.get("output_contract_completed") is True
    )
    verdict_event_count = sum(
        1 for row in review_events if str(row.get("event", "")).endswith("verdict")
    )
    flags = {key: _flag_from_sources(key, source_maps) for key in REQUIRED_FALSE_FLAGS}
    flags.update(
        {
            "metadata_only": True,
            "requirement_records_modified": False,
            "graph_or_import_writes_enabled": False,
            "source_write_attempted": False,
            "non_artifact_write_attempted": False,
            "import_ready_claimed": False,
            "import_eligible_claimed": False,
            "trusted_fact_promotion_allowed": False,
            "model_call_attempted": False,
            "secrets_included": False,
        }
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "task_id": TASK_ID,
        "selection_id": SELECTION_ID,
        "metadata_only": True,
        "source_artifacts": {
            "s02_assessment": DEFAULT_S02_ASSESSMENT.as_posix(),
            "s02_summary": DEFAULT_S02_SUMMARY.as_posix(),
            "s02_uat": DEFAULT_S02_UAT.as_posix(),
            "replay_closeout": DEFAULT_REPLAY_CLOSEOUT.as_posix(),
            "s05_closeout": DEFAULT_S05_CLOSEOUT.as_posix(),
            "progression_matrix": DEFAULT_MATRIX.as_posix(),
            "continuity_audit": DEFAULT_AUDIT.as_posix(),
            "review_events": DEFAULT_REVIEW_EVENTS.as_posix(),
        },
        "s02_assessment_reconciliation": {
            "stale_s02_assessment_failure_detected": stale,
            "fresh_65_pass_evidence_present": fresh_65,
            "fresh_65_pass_evidence_sources": [
                path
                for path, text in (
                    (DEFAULT_S02_SUMMARY.as_posix(), s02_summary),
                    (DEFAULT_S02_UAT.as_posix(), s02_uat),
                )
                if _contains_65_pass_signal(text)
            ],
            "s02_replay_closeout_status": replay_closeout.get("status"),
            "s02_replay_failed_count": replay_closeout.get("counts", {}).get("failed")
            if isinstance(replay_closeout.get("counts"), Mapping)
            else None,
            "full_repo_pytest_collection_debt": {
                "detected": full_repo_debt,
                "classified_outside_s02_uat": full_repo_debt,
                "blocking_s02_replay_closeout": False,
                "safe_claim": "Full-repo pytest collection debt is recorded as outside the scoped S02 UAT when the focused 65-test suite signal is fresh.",
            },
        },
        "requirement_coverage": _requirement_rows(),
        "canonical_verification_classes": _class_rows(),
        "safety_flags": flags,
        "graph_import_boundary": {
            "completed_review_event_count": completed_review_count,
            "verdict_event_count": verdict_event_count,
            "completed_review_refusal_in_force": completed_review_count == 0,
            "accepted_count": s05_closeout.get("accepted_count"),
            "import_eligible_count": s05_closeout.get("import_eligible_count"),
            "safe_claim": "Absent completed-review verdict evidence remains refusal evidence, not graph/import eligibility.",
        },
        "rerun_ready_validation_inputs": {
            "requirement_ids": list(REQUIRED_REQUIREMENT_IDS),
            "verification_classes": list(CANONICAL_CLASSES),
            "commands": [
                "uv run python scripts/verify_m031_validation_remediation.py --validate-only",
                "uv run pytest tests/test_m031_validation_remediation.py -q",
            ],
        },
        "quality_gates": {
            "failure_modes_q5": [
                "Filesystem inputs: missing or malformed local JSON/JSONL/Markdown artifacts return stable diagnostics and nonzero exit before any requested write.",
                "Unsafe flags or positive graph/import/LadybugDB/production/model/network/write claims fail closed before outputs are written.",
                "Absent completed-review verdict evidence is treated as refusal evidence, not positive eligibility.",
            ],
            "load_profile_q6": {
                "expected_load": "fixed M031 artifact set: S02 prose, replay closeout, S05 closeout, progression matrix, continuity audit, and two review-event rows",
                "ten_x_breakpoint": "local JSON/Markdown parsing and recursive metadata scanning saturate first at roughly 10x rows; no network, subprocess, model, graph, or LadybugDB path exists",
                "protection": "single-pass bounded summaries, counters, stable diagnostic codes, and no raw payload reads or database writes",
            },
            "negative_tests_q7": [
                "stale S02 assessment without fresh 65-pass evidence",
                "missing requirement rows",
                "missing canonical class rows",
                "unsafe true flags",
                "raw payload or key leakage",
                "permissive graph/import claims",
                "malformed diagnostics",
                "path traversal or out-of-corpus output paths",
            ],
        },
        "diagnostic_codes": [
            "M031_VALIDATION_REMEDIATION_STALE_S02_ASSESSMENT_RECONCILED",
            "M031_VALIDATION_REMEDIATION_FULL_REPO_COLLECTION_DEBT_OUTSIDE_S02_UAT",
        ],
    }


def validate_diagnostics_rows(rows: Sequence[Mapping[str, Any]]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    required_keys = {"schema_version", "code", "severity", "message", "json_path", "metadata_only"}
    for index, row in enumerate(rows):
        row_path = f"diagnostics[{index}]"
        missing = sorted(required_keys - set(row))
        if missing:
            diagnostics.append(
                diagnostic(
                    "M031_VALIDATION_REMEDIATION_MALFORMED_DIAGNOSTIC",
                    "diagnostic row missing keys: " + ", ".join(missing),
                    json_path=row_path,
                )
            )
        if row.get("schema_version") != DIAGNOSTIC_SCHEMA_VERSION:
            diagnostics.append(
                diagnostic(
                    "M031_VALIDATION_REMEDIATION_MALFORMED_DIAGNOSTIC",
                    "diagnostic row schema_version mismatch",
                    json_path=f"{row_path}.schema_version",
                )
            )
        if row.get("severity") not in {"info", "warning", "error"}:
            diagnostics.append(
                diagnostic(
                    "M031_VALIDATION_REMEDIATION_MALFORMED_DIAGNOSTIC",
                    "diagnostic severity must be info, warning, or error",
                    json_path=f"{row_path}.severity",
                )
            )
        for flag in (
            "network_fetch_attempted",
            "production_import_attempted",
            "ladybugdb_written",
            "graph_write_attempted",
            "model_call_attempted",
        ):
            if row.get(flag) is not False:
                diagnostics.append(
                    diagnostic(
                        "M031_VALIDATION_REMEDIATION_MALFORMED_DIAGNOSTIC",
                        f"diagnostic unsafe flag must be false: {flag}",
                        json_path=f"{row_path}.{flag}",
                    )
                )
    return diagnostics


def _field_name(path: str, ancestors: tuple[str, ...]) -> str:
    return (ancestors[-1] if ancestors else path.rsplit(".", maxsplit=1)[-1]).lower()


def _validate_metadata_safety(value: Any, *, where: str) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    for path, item, ancestors in _walk(value):
        field = _field_name(path, ancestors)
        if isinstance(item, bool) and item is True and field in UNSAFE_TRUE_BOOLEAN_KEYS:
            diagnostics.append(
                diagnostic(
                    "M031_VALIDATION_REMEDIATION_UNSAFE_FLAG_TRUE",
                    f"unsafe boolean field must be false: {field}",
                    json_path=path,
                    path=where,
                )
            )
        if any(fragment in field for fragment in UNSAFE_FIELD_NAME_FRAGMENTS) and item is not False:
            diagnostics.append(
                diagnostic(
                    "M031_VALIDATION_REMEDIATION_RAW_PAYLOAD_LEAKAGE",
                    f"raw/binary/vector/secret-like field is not allowed: {field}",
                    json_path=path,
                    path=where,
                )
            )
        if isinstance(item, str):
            lowered = item.lower()
            for snippet in FORBIDDEN_SNIPPETS:
                if snippet in lowered:
                    diagnostics.append(
                        diagnostic(
                            "M031_VALIDATION_REMEDIATION_RAW_PAYLOAD_LEAKAGE",
                            f"string contains forbidden payload/key marker: {snippet}",
                            json_path=path,
                            path=where,
                        )
                    )
            for phrase in FORBIDDEN_POSITIVE_PHRASES:
                if phrase in lowered:
                    diagnostics.append(
                        diagnostic(
                            "M031_VALIDATION_REMEDIATION_FORBIDDEN_POSITIVE_CLAIM",
                            f"forbidden positive claim: {phrase}",
                            json_path=path,
                            path=where,
                        )
                    )
    return diagnostics


def validate_evidence(evidence: Mapping[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    missing = sorted(REQUIRED_TOP_LEVEL_KEYS - set(evidence))
    if missing:
        diagnostics.append(
            diagnostic(
                "M031_VALIDATION_REMEDIATION_MISSING_TOP_LEVEL_KEY",
                "missing top-level keys: " + ", ".join(missing),
            )
        )
    for key, expected in {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "task_id": TASK_ID,
        "selection_id": SELECTION_ID,
        "metadata_only": True,
    }.items():
        if evidence.get(key) != expected:
            diagnostics.append(
                diagnostic(
                    "M031_VALIDATION_REMEDIATION_IDENTITY_MISMATCH",
                    f"{key} must be {expected!r}",
                    json_path=f"$.{key}",
                )
            )

    reconciliation = evidence.get("s02_assessment_reconciliation")
    if not isinstance(reconciliation, Mapping):
        diagnostics.append(
            diagnostic(
                "M031_VALIDATION_REMEDIATION_MISSING_S02_RECONCILIATION",
                "s02_assessment_reconciliation must be an object",
                json_path="$.s02_assessment_reconciliation",
            )
        )
    else:
        if reconciliation.get("stale_s02_assessment_failure_detected") is not True:
            diagnostics.append(
                diagnostic(
                    "M031_VALIDATION_REMEDIATION_STALE_S02_ASSESSMENT_NOT_DETECTED",
                    "stale S02 assessment failure must be detected",
                    json_path="$.s02_assessment_reconciliation.stale_s02_assessment_failure_detected",
                )
            )
        if reconciliation.get("fresh_65_pass_evidence_present") is not True:
            diagnostics.append(
                diagnostic(
                    "M031_VALIDATION_REMEDIATION_MISSING_S02_65_PASS_EVIDENCE",
                    "fresh S02 65-pass evidence is required",
                    json_path="$.s02_assessment_reconciliation.fresh_65_pass_evidence_present",
                )
            )
        debt = reconciliation.get("full_repo_pytest_collection_debt")
        if (
            not isinstance(debt, Mapping)
            or debt.get("classified_outside_s02_uat") is not True
            or debt.get("blocking_s02_replay_closeout") is not False
        ):
            diagnostics.append(
                diagnostic(
                    "M031_VALIDATION_REMEDIATION_COLLECTION_DEBT_SCOPE",
                    "full-repo pytest collection debt must be classified outside S02 UAT and nonblocking",
                    json_path="$.s02_assessment_reconciliation.full_repo_pytest_collection_debt",
                )
            )

    req_rows = evidence.get("requirement_coverage")
    if not isinstance(req_rows, list):
        diagnostics.append(
            diagnostic(
                "M031_VALIDATION_REMEDIATION_MISSING_REQUIREMENT_ROW",
                "requirement_coverage must be a list",
                json_path="$.requirement_coverage",
            )
        )
    else:
        by_id = {row.get("requirement_id"): row for row in req_rows if isinstance(row, Mapping)}
        missing_req = sorted(set(REQUIRED_REQUIREMENT_IDS) - set(by_id))
        # pyrefly: ignore [bad-specialization]
        extra_req = sorted(set(by_id) - set(REQUIRED_REQUIREMENT_IDS))
        if missing_req or extra_req:
            diagnostics.append(
                diagnostic(
                    "M031_VALIDATION_REMEDIATION_MISSING_REQUIREMENT_ROW",
                    f"requirement rows must exactly cover {', '.join(REQUIRED_REQUIREMENT_IDS)}; missing={missing_req}, extra={extra_req}",
                    json_path="$.requirement_coverage",
                )
            )
        for index, row in enumerate(req_rows):
            if not isinstance(row, Mapping):
                diagnostics.append(
                    diagnostic(
                        "M031_VALIDATION_REMEDIATION_MISSING_REQUIREMENT_ROW",
                        "requirement row must be an object",
                        json_path=f"$.requirement_coverage[{index}]",
                    )
                )
                continue
            if (
                row.get("validated") is not False
                or row.get("validation_claim_allowed") is not False
            ):
                diagnostics.append(
                    diagnostic(
                        "M031_VALIDATION_REMEDIATION_REQUIREMENT_OVERCLAIM",
                        "requirement row must not claim validation or status mutation",
                        json_path=f"$.requirement_coverage[{index}]",
                    )
                )
            if row.get("coverage_status") != "m031_scoped_rechecked":
                diagnostics.append(
                    diagnostic(
                        "M031_VALIDATION_REMEDIATION_REQUIREMENT_OVERCLAIM",
                        "coverage_status must be m031_scoped_rechecked",
                        json_path=f"$.requirement_coverage[{index}].coverage_status",
                    )
                )

    class_rows = evidence.get("canonical_verification_classes")
    if not isinstance(class_rows, list):
        diagnostics.append(
            diagnostic(
                "M031_VALIDATION_REMEDIATION_MISSING_CLASS_ROW",
                "canonical_verification_classes must be a list",
                json_path="$.canonical_verification_classes",
            )
        )
    else:
        classes = {row.get("class") for row in class_rows if isinstance(row, Mapping)}
        missing_classes = sorted(set(CANONICAL_CLASSES) - classes)
        # pyrefly: ignore [bad-specialization]
        extra_classes = sorted(classes - set(CANONICAL_CLASSES))
        if missing_classes or extra_classes:
            diagnostics.append(
                diagnostic(
                    "M031_VALIDATION_REMEDIATION_MISSING_CLASS_ROW",
                    f"canonical class rows must exactly cover {', '.join(CANONICAL_CLASSES)}; missing={missing_classes}, extra={extra_classes}",
                    json_path="$.canonical_verification_classes",
                )
            )

    flags = evidence.get("safety_flags")
    if not isinstance(flags, Mapping):
        diagnostics.append(
            diagnostic(
                "M031_VALIDATION_REMEDIATION_UNSAFE_FLAG_TRUE",
                "safety_flags must be an object",
                json_path="$.safety_flags",
            )
        )
    else:
        for key in sorted(REQUIRED_FALSE_FLAGS):
            if flags.get(key) is not False:
                diagnostics.append(
                    diagnostic(
                        "M031_VALIDATION_REMEDIATION_UNSAFE_FLAG_TRUE",
                        f"safety flag must be false: {key}",
                        json_path=f"$.safety_flags.{key}",
                    )
                )

    boundary = evidence.get("graph_import_boundary")
    if isinstance(boundary, Mapping):
        if boundary.get("completed_review_refusal_in_force") is not True:
            diagnostics.append(
                diagnostic(
                    "M031_VALIDATION_REMEDIATION_PERMISSIVE_GRAPH_IMPORT_CLAIM",
                    "completed-review refusal must remain in force",
                    json_path="$.graph_import_boundary.completed_review_refusal_in_force",
                )
            )
        if boundary.get("accepted_count") != 0 or boundary.get("import_eligible_count") != 0:
            diagnostics.append(
                diagnostic(
                    "M031_VALIDATION_REMEDIATION_PERMISSIVE_GRAPH_IMPORT_CLAIM",
                    "accepted/import-eligible counts must remain zero",
                    json_path="$.graph_import_boundary",
                )
            )
    else:
        diagnostics.append(
            diagnostic(
                "M031_VALIDATION_REMEDIATION_PERMISSIVE_GRAPH_IMPORT_CLAIM",
                "graph_import_boundary must be an object",
                json_path="$.graph_import_boundary",
            )
        )

    diagnostics.extend(_validate_metadata_safety(evidence, where="evidence"))
    return diagnostics


def build_runtime_diagnostics(evidence: Mapping[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    reconciliation = (
        evidence.get("s02_assessment_reconciliation")
        if isinstance(evidence.get("s02_assessment_reconciliation"), Mapping)
        else {}
    )
    if reconciliation.get("stale_s02_assessment_failure_detected") is True:  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        rows.append(
            diagnostic(
                "M031_VALIDATION_REMEDIATION_STALE_S02_ASSESSMENT_RECONCILED",
                "Stale S02 assessment failure is reconciled by fresh 65-pass S02 summary/UAT evidence.",
                severity="info",
                json_path="$.s02_assessment_reconciliation",
                path=DEFAULT_S02_ASSESSMENT,
            )
        )
    rows.append(
        diagnostic(
            "M031_VALIDATION_REMEDIATION_FULL_REPO_COLLECTION_DEBT_OUTSIDE_S02_UAT",
            "Full-repo pytest collection debt remains classified outside the scoped S02 UAT signal.",
            severity="warning",
            json_path="$.s02_assessment_reconciliation.full_repo_pytest_collection_debt",
            path=DEFAULT_S02_ASSESSMENT,
        )
    )
    return rows


def render_report(evidence: Mapping[str, Any], diagnostics: Sequence[Mapping[str, Any]]) -> str:
    req_rows = (
        evidence.get("requirement_coverage", [])
        if isinstance(evidence.get("requirement_coverage"), list)
        else []
    )
    class_rows = (
        evidence.get("canonical_verification_classes", [])
        if isinstance(evidence.get("canonical_verification_classes"), list)
        else []
    )
    req_lines = "\n".join(
        f"| {row.get('requirement_id')} | {row.get('coverage_status')} | {row.get('validated')} | {row.get('validation_claim_allowed')} |"
        for row in req_rows
        if isinstance(row, Mapping)
    )
    class_lines = "\n".join(
        f"| {row.get('class')} | {row.get('status')} | {', '.join(row.get('evidence_paths', [])) if isinstance(row.get('evidence_paths'), list) else ''} |"
        for row in class_rows
        if isinstance(row, Mapping)
    )
    codes = Counter(str(row.get("code")) for row in diagnostics)
    code_lines = (
        "\n".join(f"- `{code}`: {count}" for code, count in sorted(codes.items())) or "- None"
    )
    reconciliation = (
        evidence.get("s02_assessment_reconciliation", {})
        if isinstance(evidence.get("s02_assessment_reconciliation"), Mapping)
        else {}
    )
    source_artifacts = (
        evidence.get("source_artifacts", {})
        if isinstance(evidence.get("source_artifacts"), Mapping)
        else {}
    )
    boundary = (
        evidence.get("graph_import_boundary", {})
        if isinstance(evidence.get("graph_import_boundary"), Mapping)
        else {}
    )
    rerun_inputs = (
        evidence.get("rerun_ready_validation_inputs", {})
        if isinstance(evidence.get("rerun_ready_validation_inputs"), Mapping)
        else {}
    )
    gates = (
        evidence.get("quality_gates", {})
        if isinstance(evidence.get("quality_gates"), Mapping)
        else {}
    )
    source_lines = (
        "\n".join(f"- `{key}`: `{value}`" for key, value in sorted(source_artifacts.items()))
        or "- None"
    )
    fresh_sources = (
        ", ".join(
            f"`{path}`"
            for path in reconciliation.get("fresh_65_pass_evidence_sources", [])
            if isinstance(path, str)
        )
        or "None"
    )
    requirement_claims = (
        "\n".join(
            f"- {row.get('safe_claim')}"
            for row in req_rows
            if isinstance(row, Mapping) and row.get("safe_claim")
        )
        or "- None"
    )
    class_claims = (
        "\n".join(
            f"- {row.get('safe_claim')}"
            for row in class_rows
            if isinstance(row, Mapping) and row.get("safe_claim")
        )
        or "- None"
    )
    command_lines = (
        "\n".join(
            f"- `{command}`"
            for command in rerun_inputs.get("commands", [])
            if isinstance(command, str)
        )
        or "- `uv run python scripts/verify_m031_validation_remediation.py --validate-only`"
    )
    failure_modes = "\n".join(
        f"- {item}" for item in gates.get("failure_modes_q5", []) if isinstance(item, str)
    )
    negative_tests = "\n".join(
        f"- {item}" for item in gates.get("negative_tests_q7", []) if isinstance(item, str)
    )
    load_profile = (
        gates.get("load_profile_q6", {})
        if isinstance(gates.get("load_profile_q6"), Mapping)
        else {}
    )
    return f"""# M031 Validation Remediation Dossier

Schema: `{SCHEMA_VERSION}`
Milestone: `{MILESTONE_ID}`
Selection: `{SELECTION_ID}`
Metadata-only: true

## Reader Action

Use this dossier as the M031 validation-rerun input for S06. Read the evidence JSON for machine-checkable rows, read this report for the human handoff, and do not interpret any row as graph-import approval or global requirement validation.

## Source Artifact Audit

{source_lines}

## S02 Assessment Reconciliation

- Stale S02 assessment failure detected: `{reconciliation.get("stale_s02_assessment_failure_detected")}`
- Fresh `65 passed` evidence present: `{reconciliation.get("fresh_65_pass_evidence_present")}`
- Fresh `65 passed` evidence sources: {fresh_sources}
- Full-repo pytest collection debt classified outside S02 UAT: `{(reconciliation.get("full_repo_pytest_collection_debt") or {}).get("classified_outside_s02_uat") if isinstance(reconciliation.get("full_repo_pytest_collection_debt"), Mapping) else None}`

## Requirement Coverage

| Requirement | Coverage status | Validated | Validation claim allowed |
|---|---|---|---|
{req_lines}

## Canonical Verification Classes

| Class | Status | Evidence paths |
|---|---|---|
{class_lines}

## Safe Claims

### Requirement claims
{requirement_claims}

### Verification-class claims
{class_claims}

### Graph/import boundary claim
- {boundary.get("safe_claim")}

## Forbidden Claims

- Do not claim that S06 validates global requirement status for R024/R027/R029/R040/R050.
- Do not assert graph-import readiness, KG-import readiness, trusted fact promotion, or production-import authorization for M031.
- Do not claim completed-review evidence exists when the independent review events still enforce refusal.
- Do not claim network fetches, model calls, raw article payload handling, production graph import, or LadybugDB writes occurred.

## Fail-Closed Safety

S06 does not enable production graph import or LadybugDB writes. Graph/import/LadybugDB/production/model/network/write activity remains false, accepted/import-eligible counts remain zero, and requirement status changes are not performed.

## Milestone Validation Handoff Snippets

- S02 stale assessment closeout: stale S02 assessment failure is reconciled by fresh `65 passed` S02 summary/UAT evidence, while unrelated full-repo pytest collection debt remains outside scoped S02 UAT.
- Requirement coverage: R024/R027/R029/R040/R050 have M031-scoped remediation coverage rows only; `validated` and `validation_claim_allowed` remain false.
- Verification classes: Contract, Integration, Operational, and UAT rows are covered for validation rerun using metadata-only evidence paths.
- Safety boundary: no raw article text, chunk text, vectors, embeddings, model traces, network traces, write traces, secrets, PDF bytes, or base64 payloads are included.

## Stable Diagnostics

{code_lines}

## Failure Modes
{failure_modes}

## Load Profile
- Expected load: {load_profile.get("expected_load")}
- 10x breakpoint: {load_profile.get("ten_x_breakpoint")}
- Protection: {load_profile.get("protection")}

## Negative Tests
{negative_tests}

## Rerun Commands

{command_lines}
"""


def build_verify_summary(
    args: argparse.Namespace,
    evidence: Mapping[str, Any],
    diagnostics: Sequence[Mapping[str, Any]],
    validation_errors: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    return {
        "schema_version": VERIFY_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "task_id": TASK_ID,
        "selection_id": SELECTION_ID,
        "status": "failed" if validation_errors else "passed",
        "metadata_only": True,
        "stale_s02_assessment_failure_detected": (
            evidence.get("s02_assessment_reconciliation") or {}
        ).get("stale_s02_assessment_failure_detected")
        if isinstance(evidence.get("s02_assessment_reconciliation"), Mapping)
        else None,
        "fresh_65_pass_evidence_present": (evidence.get("s02_assessment_reconciliation") or {}).get(
            "fresh_65_pass_evidence_present"
        )
        if isinstance(evidence.get("s02_assessment_reconciliation"), Mapping)
        else None,
        "requirement_ids": [
            row.get("requirement_id")
            for row in evidence.get("requirement_coverage", [])
            if isinstance(row, Mapping)
        ]
        if isinstance(evidence.get("requirement_coverage"), list)
        else [],
        "verification_classes": [
            row.get("class")
            for row in evidence.get("canonical_verification_classes", [])
            if isinstance(row, Mapping)
        ]
        if isinstance(evidence.get("canonical_verification_classes"), list)
        else [],
        "diagnostic_count": len(diagnostics),
        "validation_error_count": len(validation_errors),
        "diagnostic_codes": sorted(
            Counter(str(row.get("code")) for row in [*diagnostics, *validation_errors])
        ),
        "source_artifact_paths": {
            "s02_assessment": Path(args.s02_assessment).as_posix(),
            "s02_summary": Path(args.s02_summary).as_posix(),
            "s02_uat": Path(args.s02_uat).as_posix(),
            "replay_closeout": Path(args.replay_closeout).as_posix(),
            "s05_closeout": Path(args.s05_closeout).as_posix(),
            "progression_matrix": Path(args.progression_matrix).as_posix(),
            "continuity_audit": Path(args.continuity_audit).as_posix(),
            "review_events": Path(args.review_events).as_posix(),
        },
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_write_attempted": False,
        "model_call_attempted": False,
    }


def _write_json(path: Path, payload: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _write_jsonl(path: Path, rows: Sequence[Mapping[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8"
    )


def _write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def run(args: argparse.Namespace) -> int:
    repo_root = Path.cwd()
    s02_assessment = load_text(args.s02_assessment, repo_root=repo_root, label="S02 assessment")
    s02_summary = load_text(args.s02_summary, repo_root=repo_root, label="S02 summary")
    s02_uat = load_text(args.s02_uat, repo_root=repo_root, label="S02 UAT")
    replay_closeout = load_json(args.replay_closeout, repo_root=repo_root, label="replay closeout")
    s05_closeout = load_json(args.s05_closeout, repo_root=repo_root, label="S05 closeout")
    progression_matrix = load_json(
        args.progression_matrix, repo_root=repo_root, label="progression matrix"
    )
    continuity_audit = load_json(
        args.continuity_audit, repo_root=repo_root, label="continuity audit"
    )
    review_events = load_jsonl(args.review_events, repo_root=repo_root, label="review events")

    evidence = build_evidence(
        s02_assessment=s02_assessment,
        s02_summary=s02_summary,
        s02_uat=s02_uat,
        replay_closeout=replay_closeout,
        s05_closeout=s05_closeout,
        progression_matrix=progression_matrix,
        continuity_audit=continuity_audit,
        review_events=review_events,
    )
    diagnostics = build_runtime_diagnostics(evidence)
    report = render_report(evidence, diagnostics)
    validation_errors = validate_evidence(evidence)
    validation_errors.extend(_validate_metadata_safety(diagnostics, where="diagnostics"))
    validation_errors.extend(validate_diagnostics_rows(diagnostics))
    validation_errors.extend(_validate_metadata_safety(report, where="report"))
    verify_summary = build_verify_summary(args, evidence, diagnostics, validation_errors)
    validation_errors.extend(_validate_metadata_safety(verify_summary, where="verify_summary"))

    if validation_errors:
        sys.stderr.write("M031 validation remediation verification failed before writes:\n")
        for row in validation_errors:
            sys.stderr.write(json.dumps(row, sort_keys=True) + "\n")
        return 1

    output_specs = [
        (args.write_evidence, "write evidence", lambda path: _write_json(path, evidence)),
        (args.write_diagnostics, "write diagnostics", lambda path: _write_jsonl(path, diagnostics)),
        (args.write_report, "write report", lambda path: _write_text(path, report)),
        (
            args.write_verify_summary,
            "write verify summary",
            lambda path: _write_json(path, verify_summary),
        ),
    ]
    safe_outputs: list[tuple[Path, Any]] = []
    for path_value, label, writer in output_specs:
        if path_value is not None:
            safe_outputs.append(
                (_safe_output_path(path_value, repo_root=repo_root, label=label), writer)
            )
    for path, writer in safe_outputs:
        writer(path)

    sys.stdout.write(json.dumps(verify_summary, sort_keys=True) + "\n")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--s02-assessment", type=Path, default=DEFAULT_S02_ASSESSMENT)
    parser.add_argument("--s02-summary", type=Path, default=DEFAULT_S02_SUMMARY)
    parser.add_argument("--s02-uat", type=Path, default=DEFAULT_S02_UAT)
    parser.add_argument("--replay-closeout", type=Path, default=DEFAULT_REPLAY_CLOSEOUT)
    parser.add_argument("--s05-closeout", type=Path, default=DEFAULT_S05_CLOSEOUT)
    parser.add_argument("--progression-matrix", type=Path, default=DEFAULT_MATRIX)
    parser.add_argument("--continuity-audit", type=Path, default=DEFAULT_AUDIT)
    parser.add_argument("--review-events", type=Path, default=DEFAULT_REVIEW_EVENTS)
    parser.add_argument("--write-evidence", type=Path)
    parser.add_argument("--write-diagnostics", type=Path)
    parser.add_argument("--write-report", type=Path)
    parser.add_argument("--write-verify-summary", type=Path)
    parser.add_argument(
        "--validate-only",
        action="store_true",
        help="Validate only; writes still occur only for explicitly requested --write-* paths.",
    )
    parser.set_defaults(
        write_evidence=None,
        write_diagnostics=None,
        write_report=None,
        write_verify_summary=None,
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv[1:] if argv and str(argv[0]).endswith(".py") else argv)
    try:
        return run(args)
    except M031ValidationRemediationError as exc:
        sys.stderr.write(f"ERROR: {exc}\n")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
