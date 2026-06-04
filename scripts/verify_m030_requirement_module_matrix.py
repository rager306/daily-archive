#!/usr/bin/env python3
"""Validate the M030 requirement-to-module coverage matrix safety contract.

This verifier is intentionally local-only. It checks the static requirement
coverage JSON and synchronized Markdown report without replaying acquisition,
parser conversion, chunking, graph-readiness review, LadybugDB writes, model
helper activation, optimizer activation, or production import.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

EXPECTED_SCHEMA_VERSION = "m030-requirement-module-matrix.v1"
EXPECTED_MILESTONE_ID = "M030-abwhdm"
EXPECTED_SLICE_ID = "S04"
EXPECTED_REQUIREMENTS = {
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
    "R036",
    "R040",
    "R050",
    "R051",
    "R052",
}
REQUIRED_SOURCE_INPUTS = {
    "doc/validation/m027_requirement_scope_matrix.json",
    "doc/validation/m028_requirement_scope_matrix.json",
    ".gsd/REQUIREMENTS.md",
    "doc/architecture/m030_pipeline_module_inventory.json",
    "doc/architecture/m030_module_function_readiness.json",
}
REQUIRED_MODULE_STAGES = {
    "url_intake",
    "article_catalog",
    "source_acquisition",
    "loader_evidence",
    "parser_conversion",
    "chunking",
    "graph_readiness_review",
    "graph_import_boundary",
    "cross_stage_replay",
}
ALLOWED_COVERAGE_STATUSES = {
    "covered",
    "supported",
    "partial",
    "blocked",
    "future_out_of_scope",
    "gated_future_scope",
    "unsafe_to_claim",
}
FUTURE_SCOPE_STATUSES = {"future_out_of_scope", "gated_future_scope"}
FAIL_CLOSED_SCOPE_FLAGS = {
    "metadata_only": True,
    "behavior_changed": False,
    "runtime_replay_performed": False,
    "network_fetch_attempted": False,
    "graph_write_attempted": False,
    "production_persistence_attempted": False,
    "readiness_claimed": False,
}
REQUIRED_REPORT_PHRASES = (
    "# M030 Requirement Module Matrix",
    "Machine-readable matrix: `doc/architecture/m030_requirement_module_matrix.json`",
    "Behavior changed: `false`",
    "Runtime replay performed: `false`",
    "## Requirement Coverage Crosswalk",
    "### Requirement safety rules",
    "not validated by M030",
    "must not claim LadybugDB writes",
)
UNSAFE_POSITIVE_CLAIM_PATTERNS = (
    r"\bvalidated\b",
    r"\bfully validates\b",
    r"\bready\b",
    r"\bimport[-_ ]?ready\s*=\s*true\b",
    r"\bimport[-_ ]?eligible\s*=\s*true\b",
    r"\bkg[-_ ]?readiness\b",
    r"\bgraph[-_ ]?ready\b",
    r"\bparser[-_ ]?ready\b",
    r"\bchunk[-_ ]?ready\b",
    r"\bladybugdb\s+written\b",
    r"\bproduction\s+import\b",
    r"\btrusted\s+fact\s+promotion\b",
    r"\bdspy\s+activation\b",
    r"\brlm\s+activation\b",
    r"\bminimax\s+activation\b",
    r"\boptimizer\s+activation\b",
)
NEGATING_SAFETY_TERMS = (
    "do not claim",
    "must not claim",
    "cannot claim",
    "not validated",
    "not ready",
    "unsafe-to-claim",
    "fail-closed",
    "blocked",
    "future-scope",
    "false",
    "without",
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


def _row_name(row: dict[str, Any], index: int) -> str:
    requirement_id = row.get("requirement_id")
    return requirement_id if isinstance(requirement_id, str) and requirement_id else f"requirements[{index}]"


def _path_from_reference(reference: str) -> Path:
    path_part = reference.split(":", 1)[0] if ".py:" in reference else reference
    return Path(path_part)


def _row_text(row: dict[str, Any]) -> str:
    return json.dumps(row, sort_keys=True).lower()


def _has_unsafe_positive_claim(row: dict[str, Any]) -> bool:
    text = _row_text(row)
    if not any(re.search(pattern, text) for pattern in UNSAFE_POSITIVE_CLAIM_PATTERNS):
        return False
    return not any(term in text for term in NEGATING_SAFETY_TERMS)


def _scope_matrix_requirement_ids(path: Path) -> set[str]:
    payload = _load_json(path)
    ids: set[str] = set()

    def walk(value: Any) -> None:
        if isinstance(value, dict):
            requirement_id = value.get("requirement_id") or value.get("id")
            if isinstance(requirement_id, str) and re.fullmatch(r"R\d{3}", requirement_id):
                ids.add(requirement_id)
            for nested in value.values():
                walk(nested)
        elif isinstance(value, list):
            for item in value:
                walk(item)

    walk(payload)
    return ids


def validate_matrix(matrix: dict[str, Any], *, base_dir: Path = Path(".")) -> list[str]:
    errors: list[str] = []
    if matrix.get("schema_version") != EXPECTED_SCHEMA_VERSION:
        errors.append("M030_REQUIREMENT_MATRIX_SCHEMA: unexpected schema_version")
    if matrix.get("milestone_id") != EXPECTED_MILESTONE_ID:
        errors.append("M030_REQUIREMENT_MATRIX_MILESTONE: unexpected milestone_id")
    if matrix.get("slice_id") != EXPECTED_SLICE_ID:
        errors.append("M030_REQUIREMENT_MATRIX_SLICE: unexpected slice_id")

    source_inputs = set(_as_nonempty_str_list(matrix.get("source_inputs")))
    missing_sources = sorted(REQUIRED_SOURCE_INPUTS - source_inputs)
    if missing_sources:
        errors.append(f"M030_REQUIREMENT_MATRIX_SOURCES: missing source inputs {missing_sources}")

    scope_boundary = matrix.get("scope_boundary")
    if not isinstance(scope_boundary, dict):
        errors.append("M030_REQUIREMENT_MATRIX_SCOPE: scope_boundary must be an object")
    else:
        for flag, expected in FAIL_CLOSED_SCOPE_FLAGS.items():
            if scope_boundary.get(flag) is not expected:
                errors.append(f"M030_REQUIREMENT_MATRIX_SCOPE: {flag} must be {expected!r}")

    legend = matrix.get("coverage_status_legend")
    if not isinstance(legend, dict):
        errors.append("M030_REQUIREMENT_MATRIX_LEGEND: coverage_status_legend must be an object")
    else:
        missing_statuses = sorted(ALLOWED_COVERAGE_STATUSES - set(legend))
        if missing_statuses:
            errors.append(f"M030_REQUIREMENT_MATRIX_LEGEND: missing statuses {missing_statuses}")

    module_refs = matrix.get("module_catalog_refs")
    if not isinstance(module_refs, list):
        errors.append("M030_REQUIREMENT_MATRIX_MODULE_REFS: module_catalog_refs must be a list")
        module_refs = []
    module_ids: set[str] = set()
    module_stages: set[str] = set()
    for index, module in enumerate(module_refs):
        if not isinstance(module, dict):
            errors.append(f"M030_REQUIREMENT_MATRIX_MODULE_REF_SHAPE: module_catalog_refs[{index}] must be an object")
            continue
        module_id = module.get("module_id")
        stage = module.get("stage")
        claim_state = module.get("claim_state")
        if not isinstance(module_id, str) or not module_id.strip():
            errors.append(f"M030_REQUIREMENT_MATRIX_MODULE_REF_ID: module_catalog_refs[{index}] missing module_id")
        else:
            if module_id in module_ids:
                errors.append(f"M030_REQUIREMENT_MATRIX_MODULE_REF_ID: duplicate {module_id}")
            module_ids.add(module_id)
        if not isinstance(stage, str) or not stage.strip():
            errors.append(f"M030_REQUIREMENT_MATRIX_MODULE_REF_STAGE: {module_id or index} missing stage")
        else:
            module_stages.add(stage)
        if not isinstance(claim_state, str) or not claim_state.strip():
            errors.append(f"M030_REQUIREMENT_MATRIX_MODULE_REF_CLAIM: {module_id or index} missing claim_state")
    missing_stages = sorted(REQUIRED_MODULE_STAGES - module_stages)
    if missing_stages:
        errors.append(f"M030_REQUIREMENT_MATRIX_MODULE_REF_STAGE: missing stages {missing_stages}")

    requirements = matrix.get("requirements")
    if not isinstance(requirements, list):
        errors.append("M030_REQUIREMENT_MATRIX_REQUIREMENTS: requirements must be a list")
        requirements = []

    seen_requirements: set[str] = set()
    for index, row in enumerate(requirements):
        if not isinstance(row, dict):
            errors.append(f"M030_REQUIREMENT_MATRIX_ROW_SHAPE: requirements[{index}] must be an object")
            continue
        row_name = _row_name(row, index)
        requirement_id = row.get("requirement_id")
        if not isinstance(requirement_id, str) or not re.fullmatch(r"R\d{3}", requirement_id):
            errors.append(f"M030_REQUIREMENT_MATRIX_ROW_ID: {row_name} missing valid requirement_id")
        else:
            if requirement_id in seen_requirements:
                errors.append(f"M030_REQUIREMENT_MATRIX_ROW_ID: duplicate {requirement_id}")
            seen_requirements.add(requirement_id)

        if row.get("current_status") not in {"active", "validated"}:
            errors.append(f"M030_REQUIREMENT_MATRIX_STATUS: {row_name} must be active or validated")
        coverage_status = row.get("coverage_status")
        if coverage_status not in ALLOWED_COVERAGE_STATUSES:
            errors.append(f"M030_REQUIREMENT_MATRIX_COVERAGE: {row_name} invalid coverage_status {coverage_status!r}")
        for field in ("classification", "requirement_summary", "next_action"):
            value = row.get(field)
            if not isinstance(value, str) or not value.strip():
                errors.append(f"M030_REQUIREMENT_MATRIX_FIELD: {row_name} missing non-empty {field}")

        mappings = row.get("mapped_modules_functions")
        if not isinstance(mappings, list) or not mappings:
            errors.append(f"M030_REQUIREMENT_MATRIX_MODULE_LINK: {row_name} missing mapped_modules_functions")
            mappings = []
        for mapping_index, mapping in enumerate(mappings):
            if not isinstance(mapping, dict):
                errors.append(
                    f"M030_REQUIREMENT_MATRIX_MODULE_LINK: {row_name} mapping {mapping_index} must be an object"
                )
                continue
            module_id = mapping.get("module_id")
            if not isinstance(module_id, str) or not module_id.strip():
                errors.append(f"M030_REQUIREMENT_MATRIX_MODULE_LINK: {row_name} mapping {mapping_index} missing module_id")
            if not isinstance(mapping.get("stage"), str) or not mapping["stage"].strip():
                errors.append(f"M030_REQUIREMENT_MATRIX_MODULE_LINK: {row_name} mapping {module_id} missing stage")
            if not isinstance(mapping.get("claim_state"), str) or not mapping["claim_state"].strip():
                errors.append(f"M030_REQUIREMENT_MATRIX_MODULE_LINK: {row_name} mapping {module_id} missing claim_state")
            files = _as_nonempty_str_list(mapping.get("files"))
            functions_classes = _as_nonempty_str_list(mapping.get("functions_classes"))
            if not files:
                errors.append(f"M030_REQUIREMENT_MATRIX_MODULE_FILES: {row_name} mapping {module_id} missing files")
            if not functions_classes:
                errors.append(
                    f"M030_REQUIREMENT_MATRIX_MODULE_FUNCTIONS: {row_name} mapping {module_id} missing functions/classes"
                )
            for reference in [*files, *functions_classes]:
                path = _path_from_reference(reference)
                if path.suffix == ".py" and not (base_dir / path).exists():
                    errors.append(f"M030_REQUIREMENT_MATRIX_MODULE_PATH: {row_name} missing {path}")

        evidence_paths = _as_nonempty_str_list(row.get("evidence_paths"))
        if not evidence_paths:
            errors.append(f"M030_REQUIREMENT_MATRIX_EVIDENCE: {row_name} missing evidence_paths")
        for evidence_path in evidence_paths:
            if not (base_dir / evidence_path).exists():
                errors.append(f"M030_REQUIREMENT_MATRIX_EVIDENCE_PATH: {row_name} missing {evidence_path}")

        unsafe_claims = _as_nonempty_str_list(row.get("unsafe_claims_to_preserve"))
        if coverage_status in FUTURE_SCOPE_STATUSES and not unsafe_claims:
            errors.append(f"M030_REQUIREMENT_MATRIX_UNSAFE_CLAIMS: {row_name} future-scope row must preserve unsafe claims")
        if coverage_status in FUTURE_SCOPE_STATUSES and _has_unsafe_positive_claim(row):
            errors.append(f"M030_REQUIREMENT_MATRIX_UNSAFE_POSITIVE: {row_name} contains unsafe positive claim")
        if coverage_status == "unsafe_to_claim" and "unsafe" not in _row_text(row) and "fail-closed" not in _row_text(row):
            errors.append(f"M030_REQUIREMENT_MATRIX_UNSAFE_BOUNDARY: {row_name} lacks unsafe/fail-closed language")

    if seen_requirements != EXPECTED_REQUIREMENTS:
        errors.append(
            "M030_REQUIREMENT_MATRIX_REQUIRED_ROWS: "
            f"expected {sorted(EXPECTED_REQUIREMENTS)}, got {sorted(seen_requirements)}"
        )

    for scope_path in (
        base_dir / "doc/validation/m027_requirement_scope_matrix.json",
        base_dir / "doc/validation/m028_requirement_scope_matrix.json",
    ):
        try:
            scope_ids = _scope_matrix_requirement_ids(scope_path)
        except ValueError as exc:
            errors.append(f"M030_REQUIREMENT_MATRIX_SCOPE_INPUT: {exc}")
            continue
        missing_from_matrix = sorted((scope_ids & EXPECTED_REQUIREMENTS) - seen_requirements)
        if missing_from_matrix:
            errors.append(f"M030_REQUIREMENT_MATRIX_SCOPE_ROWS: missing {missing_from_matrix} from {scope_path}")
    return errors


def validate_report(report_path: Path, matrix: dict[str, Any]) -> list[str]:
    text = _load_text(report_path)
    errors: list[str] = []
    for phrase in REQUIRED_REPORT_PHRASES:
        if phrase not in text:
            errors.append(f"M030_REQUIREMENT_MATRIX_REPORT_PHRASE: report missing {phrase!r}")

    for row in matrix.get("requirements", []):
        if not isinstance(row, dict):
            continue
        requirement_id = row.get("requirement_id")
        coverage_status = row.get("coverage_status")
        classification = row.get("classification")
        if isinstance(requirement_id, str) and f"`{requirement_id}`" not in text:
            errors.append(f"M030_REQUIREMENT_MATRIX_REPORT_ROW: report missing {requirement_id}")
        if isinstance(coverage_status, str) and f"`{coverage_status}`" not in text:
            errors.append(f"M030_REQUIREMENT_MATRIX_REPORT_COVERAGE: report missing {coverage_status}")
        if isinstance(classification, str) and f"`{classification}`" not in text:
            errors.append(f"M030_REQUIREMENT_MATRIX_REPORT_CLASSIFICATION: report missing {classification}")
        for mapping in row.get("mapped_modules_functions", []):
            if not isinstance(mapping, dict):
                continue
            module_id = mapping.get("module_id")
            if isinstance(module_id, str) and module_id and f"`{module_id}`" not in text:
                errors.append(f"M030_REQUIREMENT_MATRIX_REPORT_MODULE: report missing {module_id}")
        for evidence_path in _as_nonempty_str_list(row.get("evidence_paths")):
            if f"`{evidence_path}`" not in text:
                errors.append(f"M030_REQUIREMENT_MATRIX_REPORT_EVIDENCE: report missing {evidence_path}")
    return errors


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--matrix", type=Path, required=True, help="Path to requirement coverage JSON matrix")
    parser.add_argument("--report", type=Path, required=True, help="Path to requirement coverage Markdown report")
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
        base_dir = args.matrix.parent.parent.parent if args.matrix.parent.name == "architecture" else Path(".")
        errors = validate_matrix(matrix, base_dir=base_dir)
        errors.extend(validate_report(args.report, matrix))
    except ValueError as exc:
        errors = [f"M030_REQUIREMENT_MATRIX_INPUT: {exc}"]

    if errors:
        for error in errors:
            sys.stderr.write(f"{error}\n")
        return 1

    requirement_count = len(matrix.get("requirements", []))
    module_count = len(matrix.get("module_catalog_refs", []))
    sys.stdout.write(
        "M030_REQUIREMENT_MATRIX_OK: "
        f"requirements={requirement_count} modules={module_count} "
        "evidence=present report=synchronized unsafe_claims=fail_closed validate_only=true\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
