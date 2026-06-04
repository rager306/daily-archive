#!/usr/bin/env python3
"""Validate the M030 module function readiness matrix safety contract.

This verifier is intentionally local-only. It checks the static readiness JSON
and Markdown report without replaying source acquisition, parser conversion,
chunking, graph-readiness review, LadybugDB writes, DSPy/RLM/MiniMax calls, or
production import.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

EXPECTED_SCHEMA_VERSION = "m030-module-function-readiness-report.v1"
EXPECTED_MILESTONE_ID = "M030-abwhdm"
EXPECTED_SLICE_ID = "S03"
EXPECTED_SOURCE_INVENTORY = "doc/architecture/m030_pipeline_module_inventory.json"
EXPECTED_SOURCE_SELECTION = "data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json"
REQUIRED_STAGES = {
    "url_intake": "ready",
    "article_catalog": "partial",
    "source_acquisition": "blocked",
    "loader_evidence": "partial",
    "parser_conversion": "blocked",
    "chunking": "future-scope",
    "graph_readiness_review": "future-scope",
    "graph_import_boundary": "unsafe-to-claim",
    "cross_stage_replay": "blocked",
}
ALLOWED_CLAIM_STATES = {"ready", "partial", "future-scope", "blocked", "deprecated", "unsafe-to-claim"}
REQUIRED_ROW_LIST_FIELDS = {
    "primary_functions_classes",
    "tests_verifiers",
    "observability",
    "next_actions",
}
FAIL_CLOSED_SCOPE_FLAGS = {
    "behavior_changed": False,
    "runtime_replay_performed": False,
    "readiness_report_only": True,
    "graph_write_attempted": False,
    "production_persistence_attempted": False,
}
FAIL_CLOSED_SELECTION_FLAGS = {
    "loader_owns_selection": False,
    "source_acquisition_completed": False,
    "raw_article_text_embedded": False,
    "binary_payload_embedded": False,
    "parser_ready_claimed": False,
    "chunk_ready_claimed": False,
    "kg_readiness_claimed": False,
    "graph_write_attempted": False,
    "production_persistence_attempted": False,
}
UNSAFE_READY_STAGES = {
    "source_acquisition",
    "parser_conversion",
    "chunking",
    "graph_readiness_review",
    "graph_import_boundary",
    "cross_stage_replay",
}
UNSAFE_POSITIVE_TERMS = {
    "dspy",
    "rlm",
    "minimax",
    "ladybugdb",
    "graph write",
    "graph_write",
    "kg readiness",
    "kg_readiness",
    "parser ready",
    "parser_ready",
    "chunk ready",
    "chunk_ready",
    "import eligible",
    "import_eligible",
    "production import",
    "production_import",
}
FAIL_CLOSED_TERMS = {
    "fail-closed",
    "unsafe-to-claim",
    "not import-ready",
    "not_import_ready",
    "false",
    "0",
    "blocked",
    "future-scope",
}
REQUIRED_REPORT_PHRASES = (
    "# M030 Module Function Readiness Report",
    "readiness report only",
    "does **not** replay acquisition",
    "write LadybugDB",
    "claim production ingestion",
    "Safety flags: source acquisition, parser readiness, chunk readiness, KG readiness, graph writes, and production persistence remain `false`.",
    "🚫 `unsafe-to-claim`",
)


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise ValueError(f"{path}: file does not exist") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"{path}: invalid JSON: {exc}") from exc
    if not isinstance(payload, dict):
        raise ValueError(f"{path}: expected a JSON object")
    return payload


def _load_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError as exc:
        raise ValueError(f"{path}: file does not exist") from exc


def _as_nonempty_str_list(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, str) and item.strip()]


def _row_text(row: dict[str, Any]) -> str:
    return json.dumps(row, sort_keys=True).lower()


def _has_fail_closed_language(value: Any) -> bool:
    text = json.dumps(value, sort_keys=True).lower()
    return any(term in text for term in FAIL_CLOSED_TERMS)


def _contains_unsafe_positive_claim(row: dict[str, Any]) -> bool:
    text = _row_text(row)
    if not any(term in text for term in UNSAFE_POSITIVE_TERMS):
        return False
    positive_terms = (
        " ready",
        "ready ",
        "ready_claimed\": true",
        "attempted\": true",
        "completed\": true",
        "eligible\": true",
        "written\": true",
        "promoted\": true",
    )
    negative_terms = (
        "not ready",
        "not import-ready",
        "not_import_ready",
        "unsafe-to-claim",
        "must not claim",
        "cannot claim",
        "false",
        "blocked",
        "future-scope",
        "fail-closed",
        "zero",
    )
    return any(term in text for term in positive_terms) and not any(term in text for term in negative_terms)


def validate_matrix(matrix: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if matrix.get("schema_version") != EXPECTED_SCHEMA_VERSION:
        errors.append("M030_READINESS_SCHEMA: unexpected schema_version")
    if matrix.get("milestone_id") != EXPECTED_MILESTONE_ID:
        errors.append("M030_READINESS_MILESTONE: unexpected milestone_id")
    if matrix.get("slice_id") != EXPECTED_SLICE_ID:
        errors.append("M030_READINESS_SLICE: unexpected slice_id")
    if matrix.get("source_inventory") != EXPECTED_SOURCE_INVENTORY:
        errors.append("M030_READINESS_SOURCE_INVENTORY: unexpected source_inventory")
    if matrix.get("source_selection") != EXPECTED_SOURCE_SELECTION:
        errors.append("M030_READINESS_SOURCE_SELECTION: unexpected source_selection")

    scope_boundary = matrix.get("scope_boundary")
    if not isinstance(scope_boundary, dict):
        errors.append("M030_READINESS_SCOPE_BOUNDARY: scope_boundary must be an object")
    else:
        for flag, expected in FAIL_CLOSED_SCOPE_FLAGS.items():
            if scope_boundary.get(flag) is not expected:
                errors.append(f"M030_READINESS_SCOPE_BOUNDARY: {flag} must be {expected!r}")

    selection_summary = matrix.get("selection_summary")
    if not isinstance(selection_summary, dict):
        errors.append("M030_READINESS_SELECTION_SUMMARY: selection_summary must be an object")
    else:
        flags = selection_summary.get("safety_flags")
        if not isinstance(flags, dict):
            errors.append("M030_READINESS_SELECTION_FLAGS: safety_flags must be an object")
        else:
            for flag, expected in FAIL_CLOSED_SELECTION_FLAGS.items():
                if flags.get(flag) is not expected:
                    errors.append(f"M030_READINESS_SELECTION_FLAGS: {flag} must be {expected!r}")
            if flags.get("network_availability_checked") is not True:
                errors.append("M030_READINESS_SELECTION_FLAGS: network_availability_checked must be true")

    stages = matrix.get("stages")
    if not isinstance(stages, list):
        errors.append("M030_READINESS_STAGES: stages must be a list")
        stages = []

    seen_stages: set[str] = set()
    for index, row in enumerate(stages):
        if not isinstance(row, dict):
            errors.append(f"M030_READINESS_ROW_SHAPE: stages[{index}] must be an object")
            continue
        stage = row.get("stage")
        row_name = stage if isinstance(stage, str) and stage else f"stages[{index}]"
        if not isinstance(stage, str) or not stage.strip():
            errors.append(f"M030_READINESS_STAGE: {row_name} missing stage")
            continue
        if stage in seen_stages:
            errors.append(f"M030_READINESS_STAGE: duplicate stage {stage}")
        seen_stages.add(stage)
        if stage not in REQUIRED_STAGES:
            errors.append(f"M030_READINESS_STAGE: unexpected stage {stage}")

        expected_state = REQUIRED_STAGES.get(stage)
        claim_state = row.get("claim_state")
        if claim_state not in ALLOWED_CLAIM_STATES:
            errors.append(f"M030_READINESS_CLAIM_STATE: {row_name} has invalid claim_state {claim_state!r}")
        if expected_state is not None and claim_state != expected_state:
            errors.append(f"M030_READINESS_CLAIM_STATE: {row_name} expected {expected_state}, got {claim_state!r}")
        if row.get("module_inventory_status") != "covered":
            errors.append(f"M030_READINESS_INVENTORY: {row_name} module_inventory_status must be covered")

        for field in ("label", "module_id", "rationale"):
            value = row.get(field)
            if not isinstance(value, str) or not value.strip():
                errors.append(f"M030_READINESS_ROW_FIELD: {row_name} missing non-empty {field}")
        for field in sorted(REQUIRED_ROW_LIST_FIELDS):
            if not _as_nonempty_str_list(row.get(field)):
                errors.append(f"M030_READINESS_ROW_EVIDENCE: {row_name} missing non-empty {field}")

        unsafe_functions = row.get("unsafe_to_claim_functions")
        if not isinstance(unsafe_functions, list):
            errors.append(f"M030_READINESS_UNSAFE_FUNCTIONS: {row_name} unsafe_to_claim_functions must be a list")
        elif stage in UNSAFE_READY_STAGES and not _as_nonempty_str_list(unsafe_functions):
            errors.append(f"M030_READINESS_UNSAFE_FUNCTIONS: {row_name} must list unsafe-to-claim functions")

        if stage in UNSAFE_READY_STAGES and claim_state == "ready":
            errors.append(f"M030_READINESS_UNSAFE_READY: {row_name} must not be ready before replay evidence")
        if stage in UNSAFE_READY_STAGES and not _has_fail_closed_language(row):
            errors.append(f"M030_READINESS_FAIL_CLOSED: {row_name} missing fail-closed/blocked/unsafe language")
        if _contains_unsafe_positive_claim(row):
            errors.append(f"M030_READINESS_UNSAFE_POSITIVE_CLAIM: {row_name} contains an unsafe positive readiness claim")

    missing_stages = sorted(set(REQUIRED_STAGES) - seen_stages)
    if missing_stages:
        errors.append(f"M030_READINESS_STAGE_MISSING: missing stages {missing_stages}")
    return errors


def validate_report(report_path: Path, matrix: dict[str, Any]) -> list[str]:
    text = _load_text(report_path)
    errors: list[str] = []
    for phrase in REQUIRED_REPORT_PHRASES:
        if phrase not in text:
            errors.append(f"M030_READINESS_REPORT_PHRASE: report missing {phrase!r}")

    for row in matrix.get("stages", []):
        if not isinstance(row, dict):
            continue
        label = row.get("label")
        module_id = row.get("module_id")
        claim_state = row.get("claim_state")
        if isinstance(label, str) and label and label not in text:
            errors.append(f"M030_READINESS_REPORT_STAGE: report missing label {label!r}")
        if isinstance(module_id, str) and module_id and module_id not in text:
            errors.append(f"M030_READINESS_REPORT_MODULE: report missing module {module_id!r}")
        if isinstance(claim_state, str) and claim_state and f"`{claim_state}`" not in text:
            errors.append(f"M030_READINESS_REPORT_STATE: report missing claim state {claim_state!r}")
    return errors


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--matrix", type=Path, required=True, help="Path to readiness JSON matrix")
    parser.add_argument("--report", type=Path, required=True, help="Path to readiness Markdown report")
    parser.add_argument(
        "--validate-only",
        action="store_true",
        help="Validate inputs without writing output. This verifier never writes output.",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        matrix = _load_json(args.matrix)
        errors = validate_matrix(matrix)
        errors.extend(validate_report(args.report, matrix))
    except ValueError as exc:
        errors = [f"M030_READINESS_INPUT: {exc}"]

    if errors:
        for error in errors:
            sys.stderr.write(f"{error}\n")
        return 1

    stage_count = len(matrix.get("stages", []))
    sys.stdout.write(
        "M030_READINESS_OK: "
        f"stages={stage_count} required={len(REQUIRED_STAGES)} "
        "evidence=present unsafe_claims=fail_closed validate_only=true\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
