#!/usr/bin/env python3
"""Validate the M030 pipeline module inventory coverage contract.

This verifier is intentionally local-only. It validates the static module
inventory and its Markdown report without replaying source acquisition, parser
conversion, chunking, graph-readiness review, LadybugDB writes, or production
import.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

EXPECTED_SCHEMA_VERSION = "m030-pipeline-module-inventory.v1"
EXPECTED_MILESTONE_ID = "M030-abwhdm"
EXPECTED_SLICE_ID = "S02"
REQUIRED_STAGES = {
    "url_intake",
    "article_catalog",
    "source_acquisition",
    "loader_evidence",
    "parser_conversion",
    "chunking",
    "graph_readiness_review",
    "graph_import_boundary",
}
REQUIRED_ROW_FIELDS = {
    "owner_files",
    "primary_functions_classes",
    "inputs",
    "outputs",
    "tests_verifiers",
    "evidence_paths",
    "failure_modes",
    "load_profile",
    "negative_tests",
    "observability",
}
STAGE_REPORT_ALIASES = {
    "url_intake": ("URL intake", "url_intake"),
    "article_catalog": ("Article catalog", "article_catalog"),
    "source_acquisition": ("Source acquisition", "source_acquisition"),
    "loader_evidence": ("Loader evidence", "loader_evidence"),
    "parser_conversion": ("Parser/conversion", "parser_conversion"),
    "chunking": ("Chunking", "chunking"),
    "graph_readiness_review": ("Graph-readiness review", "graph_readiness_review"),
    "graph_import_boundary": ("Graph import boundary", "graph_import_boundary"),
}
REQUIRED_QUALITY_GATES = {
    "failure_modes_q5",
    "load_profile_q6",
    "negative_tests_q7",
}
FAIL_CLOSED_TERMS = (
    "fail-closed",
    "ladybugdb",
    "production import",
    "production_import_attempted",
    "ladybugdb_written",
    "import_eligible_count",
    "promoted_to_fact_count",
)


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ValueError(f"{path}: invalid JSON: {exc}") from exc
    if not isinstance(payload, dict):
        raise ValueError(f"{path}: expected a JSON object")
    return payload


def _as_nonempty_str_list(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, str) and item.strip()]


def _has_nonempty_value(value: Any) -> bool:
    if isinstance(value, str):
        return bool(value.strip())
    if isinstance(value, list):
        return bool(_as_nonempty_str_list(value))
    return False


def _contains_false_flag(payload: Any) -> bool:
    if isinstance(payload, dict):
        return any(
            (
                key in {"behavior_changed", "runtime_replay_required", "readiness_claimed"}
                and value is False
            )
            or _contains_false_flag(value)
            for key, value in payload.items()
        )
    if isinstance(payload, list):
        return any(_contains_false_flag(item) for item in payload)
    return False


def _contains_fail_closed_boundary(payload: Any) -> bool:
    text = json.dumps(payload, sort_keys=True).lower()
    return any(term in text for term in FAIL_CLOSED_TERMS) and (
        "false" in text or "0" in text or _contains_false_flag(payload)
    )


def validate_inventory(inventory: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if inventory.get("schema_version") != EXPECTED_SCHEMA_VERSION:
        errors.append("M030_PIPELINE_INVENTORY_SCHEMA: unexpected schema_version")
    if inventory.get("milestone_id") != EXPECTED_MILESTONE_ID:
        errors.append("M030_PIPELINE_INVENTORY_MILESTONE: unexpected milestone_id")
    if inventory.get("slice_id") != EXPECTED_SLICE_ID:
        errors.append("M030_PIPELINE_INVENTORY_SLICE: unexpected slice_id")

    required_stages = set(_as_nonempty_str_list(inventory.get("required_stages")))
    if required_stages != REQUIRED_STAGES:
        errors.append(
            "M030_PIPELINE_INVENTORY_REQUIRED_STAGES: "
            f"expected {sorted(REQUIRED_STAGES)}, got {sorted(required_stages)}"
        )

    modules = inventory.get("modules")
    if not isinstance(modules, list):
        errors.append("M030_PIPELINE_INVENTORY_MODULES: modules must be a list")
        modules = []
    if len(modules) < len(REQUIRED_STAGES):
        errors.append(
            "M030_PIPELINE_INVENTORY_MODULE_COUNT: "
            f"expected at least {len(REQUIRED_STAGES)} modules, got {len(modules)}"
        )

    coverage = inventory.get("stage_coverage")
    if not isinstance(coverage, dict):
        errors.append("M030_PIPELINE_INVENTORY_STAGE_COVERAGE: stage_coverage must be an object")
        coverage = {}

    module_ids: set[str] = set()
    modules_by_stage: dict[str, list[str]] = {}
    for index, row in enumerate(modules):
        if not isinstance(row, dict):
            errors.append(f"M030_PIPELINE_INVENTORY_ROW_SHAPE: modules[{index}] must be an object")
            continue
        module_id = row.get("module_id")
        stage = row.get("stage")
        if not isinstance(module_id, str) or not module_id.strip():
            errors.append(f"M030_PIPELINE_INVENTORY_MODULE_ID: modules[{index}] missing module_id")
            module_id = f"modules[{index}]"
        else:
            if module_id in module_ids:
                errors.append(f"M030_PIPELINE_INVENTORY_MODULE_ID: duplicate module_id {module_id}")
            module_ids.add(module_id)
        if not isinstance(stage, str) or not stage.strip():
            errors.append(f"M030_PIPELINE_INVENTORY_STAGE: {module_id} missing stage")
        else:
            modules_by_stage.setdefault(stage, []).append(module_id)
        if row.get("status") != "covered":
            errors.append(f"M030_PIPELINE_INVENTORY_STATUS: {module_id} must be covered")
        for field in sorted(REQUIRED_ROW_FIELDS):
            if not _has_nonempty_value(row.get(field)):
                errors.append(
                    f"M030_PIPELINE_INVENTORY_ROW_FIELD: {module_id} missing non-empty {field}"
                )

    for stage in sorted(REQUIRED_STAGES):
        covered_ids = _as_nonempty_str_list(coverage.get(stage))
        if not covered_ids:
            errors.append(
                f"M030_PIPELINE_INVENTORY_STAGE_MISSING: {stage} has no stage_coverage rows"
            )
            continue
        for module_id in covered_ids:
            if module_id not in module_ids:
                errors.append(
                    f"M030_PIPELINE_INVENTORY_STAGE_LINK: {stage} references unknown {module_id}"
                )
            if module_id not in modules_by_stage.get(stage, []):
                errors.append(
                    f"M030_PIPELINE_INVENTORY_STAGE_LINK: {module_id} is not a {stage} module row"
                )

    graph_rows = [
        row
        for row in modules
        if isinstance(row, dict) and row.get("stage") == "graph_import_boundary"
    ]
    if not graph_rows:
        errors.append(
            "M030_PIPELINE_INVENTORY_GRAPH_BOUNDARY: missing graph_import_boundary module row"
        )
    elif not any(_contains_fail_closed_boundary(row) for row in graph_rows):
        errors.append(
            "M030_PIPELINE_INVENTORY_GRAPH_BOUNDARY: graph_import_boundary lacks fail-closed false/zero boundary evidence"
        )

    scope_boundary = inventory.get("scope_boundary")
    if not isinstance(scope_boundary, dict):
        errors.append("M030_PIPELINE_INVENTORY_SCOPE_BOUNDARY: scope_boundary must be an object")
    else:
        for flag in ("behavior_changed", "runtime_replay_required", "readiness_claimed"):
            if scope_boundary.get(flag) is not False:
                errors.append(f"M030_PIPELINE_INVENTORY_SCOPE_BOUNDARY: {flag} must be false")

    quality_gates = inventory.get("quality_gates")
    if not isinstance(quality_gates, dict):
        errors.append("M030_PIPELINE_INVENTORY_QUALITY_GATES: quality_gates must be an object")
    else:
        for gate in sorted(REQUIRED_QUALITY_GATES):
            gate_payload = quality_gates.get(gate)
            if not isinstance(gate_payload, dict):
                errors.append(f"M030_PIPELINE_INVENTORY_QUALITY_GATE: {gate} must be an object")
                continue
            if gate_payload.get("verdict") != "addressed":
                errors.append(f"M030_PIPELINE_INVENTORY_QUALITY_GATE: {gate} must be addressed")
            if (
                not isinstance(gate_payload.get("summary"), str)
                or not gate_payload["summary"].strip()
            ):
                errors.append(f"M030_PIPELINE_INVENTORY_QUALITY_GATE: {gate} missing summary")
    return errors


def validate_report(report_path: Path, inventory: dict[str, Any]) -> list[str]:
    text = report_path.read_text(encoding="utf-8")
    errors: list[str] = []
    required_phrases = [
        "# M030 Pipeline Module Inventory",
        "## Stage Coverage Summary",
        "## Module Inventory",
        "## Quality Gate Notes",
        "Q5 Failure Modes",
        "Q6 Load Profile",
        "Q7 Negative Tests",
        "fail-closed",
        "LadybugDB",
        "production import",
    ]
    for phrase in required_phrases:
        if phrase not in text:
            errors.append(f"M030_PIPELINE_INVENTORY_REPORT_PHRASE: report missing {phrase!r}")

    for stage in REQUIRED_STAGES:
        aliases = STAGE_REPORT_ALIASES[stage]
        if not any(alias in text for alias in aliases):
            errors.append(f"M030_PIPELINE_INVENTORY_REPORT_STAGE: report missing stage {stage}")

    for row in inventory.get("modules", []):
        if not isinstance(row, dict):
            continue
        module_id = row.get("module_id")
        if isinstance(module_id, str) and module_id not in text:
            errors.append(
                f"M030_PIPELINE_INVENTORY_REPORT_MODULE: report missing module {module_id}"
            )
        for evidence_path in _as_nonempty_str_list(row.get("evidence_paths")):
            if evidence_path not in text:
                errors.append(
                    f"M030_PIPELINE_INVENTORY_REPORT_EVIDENCE: report missing evidence path {evidence_path}"
                )
    return errors


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inventory", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument(
        "--validate-only",
        action="store_true",
        help="Validate existing local inventory/report artifacts only.",
    )
    args = parser.parse_args(argv)
    if not args.validate_only:
        parser.error(
            "only --validate-only is supported; this verifier must not fetch, replay, or write"
        )
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    errors: list[str] = []
    try:
        inventory = _load_json(args.inventory)
        errors.extend(validate_inventory(inventory))
        errors.extend(validate_report(args.report, inventory))
    except (OSError, ValueError) as exc:
        errors.append(f"M030_PIPELINE_INVENTORY_IO: {exc}")

    if errors:
        sys.stderr.write("M030 pipeline module inventory validation failed:\n")
        for error in errors:
            sys.stderr.write(f"- {error}\n")
        return 1

    module_count = len(inventory.get("modules", []))
    required_stage_count = len(REQUIRED_STAGES)
    sys.stdout.write(
        "M030 pipeline module inventory validation passed: "
        f"module_count={module_count} required_stage_count={required_stage_count} "
        "all_required_stages_covered=true evidence_paths_present=true "
        "graph_import_boundary_fail_closed=true.\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
