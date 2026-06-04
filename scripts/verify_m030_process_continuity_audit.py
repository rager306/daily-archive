#!/usr/bin/env python3
"""Validate the M030 process continuity audit completeness contract.

This verifier is intentionally local-only. It validates the static continuity
JSON and Markdown report without registering refs, acquiring sources, parsing,
chunking, writing LadybugDB, or attempting production import.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

EXPECTED_SCHEMA_VERSION = "m030-process-continuity-audit.v1"
EXPECTED_MILESTONE_ID = "M030-abwhdm"
EXPECTED_SLICE_ID = "S05"
EXPECTED_STAGE_ORDER = [
    "url_intake",
    "article_catalog",
    "source_acquisition",
    "loader_evidence",
    "parser_conversion",
    "chunking",
    "graph_readiness_review",
    "graph_import_boundary",
]
EXPECTED_EDGES = {
    "E01_url_intake_to_article_catalog": ("url_intake", "article_catalog"),
    "E02_article_catalog_to_source_acquisition": ("article_catalog", "source_acquisition"),
    "E03_source_acquisition_to_loader_evidence": ("source_acquisition", "loader_evidence"),
    "E04_loader_evidence_to_parser_conversion": ("loader_evidence", "parser_conversion"),
    "E05_parser_conversion_to_chunking": ("parser_conversion", "chunking"),
    "E06_chunking_to_graph_readiness_review": ("chunking", "graph_readiness_review"),
    "E07_graph_readiness_review_to_graph_import_boundary": (
        "graph_readiness_review",
        "graph_import_boundary",
    ),
}
REQUIRED_EDGE_FIELDS = {
    "inputs",
    "outputs",
    "owner_modules",
    "verifiers",
    "failure_modes",
}
REQUIRED_BREAKPOINTS = {
    "B01_missing_m030_catalog_records",
    "B02_abs_without_local_pdf_artifact",
    "B03_retrieval_only_not_graph_import_ready",
}
REQUIRED_SOURCE_ARTIFACTS = {
    "data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json",
    "data/article_corpora/m028-universal-loader-runtime-smoke-v1/universal-loader-evidence-summary.json",
    "data/article_corpora/m027-mixed-source-corpus-v1/current-pipeline-baseline-summary.json",
    "data/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay-summary.json",
    "doc/architecture/m030_pipeline_module_inventory.json",
    "doc/architecture/m030_module_function_readiness.json",
}
FAIL_CLOSED_SCOPE_FLAGS = {
    "artifact_only": True,
    "behavior_changed": False,
    "runtime_replay_performed": False,
    "network_fetch_attempted": False,
    "graph_write_attempted": False,
    "production_persistence_attempted": False,
}
FAIL_CLOSED_SELECTION_FLAGS = {
    "source_acquisition_completed": False,
    "parser_ready_claimed": False,
    "chunk_ready_claimed": False,
    "kg_readiness_claimed": False,
    "graph_write_attempted": False,
    "production_persistence_attempted": False,
}
REQUIRED_REPORT_PHRASES = (
    "# M030 Process Continuity Audit",
    "## Boundary Statement",
    "## Prioritized Breaks and Gaps",
    "## Ordered Remediation Path",
    "## Unsafe Claims to Preserve",
    "Catalog/selection split",
    "Missing catalog registrations",
    "Source/PDF acquisition gaps",
    "Parser handoff gaps",
    "Zero-chunk parser-ready cases",
    "Graph-review missing prerequisites",
    "Stale hash chains",
    "Missing validators",
    "LadybugDB",
    "production persistence",
)
FAIL_CLOSED_TERMS = (
    "fail-closed",
    "unsafe_to_claim_import",
    "unsafe-to-claim",
    "graph_import_allowed=false",
    "trusted_kg_import_allowed=false",
    "ladybugdb_written=false",
    "production_import_attempted=false",
    "graph_write_attempted=false",
    "production_persistence_attempted=false",
    "import_ready=false",
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


def _has_nonempty_list(value: Any) -> bool:
    return bool(_as_nonempty_str_list(value))


def _json_text(value: Any) -> str:
    return json.dumps(value, sort_keys=True).lower()


def _contains_fail_closed_boundary(value: Any) -> bool:
    text = _json_text(value)
    return any(term in text for term in FAIL_CLOSED_TERMS) and (
        "false" in text or "0" in text or "blocked" in text or "future_scope" in text
    )


def validate_audit(audit: dict[str, Any], *, root: Path) -> list[str]:
    errors: list[str] = []
    if audit.get("schema_version") != EXPECTED_SCHEMA_VERSION:
        errors.append("M030_CONTINUITY_SCHEMA: unexpected schema_version")
    if audit.get("milestone_id") != EXPECTED_MILESTONE_ID:
        errors.append("M030_CONTINUITY_MILESTONE: unexpected milestone_id")
    if audit.get("slice_id") != EXPECTED_SLICE_ID:
        errors.append("M030_CONTINUITY_SLICE: unexpected slice_id")

    scope_boundary = audit.get("scope_boundary")
    if not isinstance(scope_boundary, dict):
        errors.append("M030_CONTINUITY_SCOPE_BOUNDARY: scope_boundary must be an object")
    else:
        for flag, expected in FAIL_CLOSED_SCOPE_FLAGS.items():
            if scope_boundary.get(flag) is not expected:
                errors.append(f"M030_CONTINUITY_SCOPE_BOUNDARY: {flag} must be {expected!r}")

    source_artifacts = audit.get("source_artifacts")
    if not isinstance(source_artifacts, list):
        errors.append("M030_CONTINUITY_SOURCE_ARTIFACTS: source_artifacts must be a list")
        source_artifacts = []
    source_paths: set[str] = set()
    for index, row in enumerate(source_artifacts):
        if not isinstance(row, dict):
            errors.append(f"M030_CONTINUITY_SOURCE_ARTIFACT_ROW: source_artifacts[{index}] must be an object")
            continue
        path = row.get("path")
        role = row.get("role")
        if not isinstance(path, str) or not path.strip():
            errors.append(f"M030_CONTINUITY_SOURCE_ARTIFACT_PATH: source_artifacts[{index}] missing path")
            continue
        source_paths.add(path)
        if not isinstance(role, str) or not role.strip():
            errors.append(f"M030_CONTINUITY_SOURCE_ARTIFACT_ROLE: {path} missing role")
        if Path(path).is_absolute():
            errors.append(f"M030_CONTINUITY_SOURCE_ARTIFACT_PATH: {path} must be relative")
        if not (root / path).exists():
            errors.append(f"M030_CONTINUITY_SOURCE_ARTIFACT_EXISTS: {path} does not exist")
    missing_sources = sorted(REQUIRED_SOURCE_ARTIFACTS - source_paths)
    if missing_sources:
        errors.append(f"M030_CONTINUITY_SOURCE_ARTIFACTS: missing required source artifacts {missing_sources}")

    selection = audit.get("input_evidence_summary", {}).get("selection") if isinstance(audit.get("input_evidence_summary"), dict) else None
    if not isinstance(selection, dict):
        errors.append("M030_CONTINUITY_SELECTION: input_evidence_summary.selection must be an object")
    else:
        if selection.get("requested_url_refs") != 4:
            errors.append("M030_CONTINUITY_SELECTION: requested_url_refs must be 4")
        if selection.get("already_in_article_catalog") != 2:
            errors.append("M030_CONTINUITY_SELECTION: already_in_article_catalog must be 2")
        if selection.get("missing_from_article_catalog") != 2:
            errors.append("M030_CONTINUITY_SELECTION: missing_from_article_catalog must be 2")
        flags = selection.get("safety_flags")
        if not isinstance(flags, dict):
            errors.append("M030_CONTINUITY_SELECTION_FLAGS: safety_flags must be an object")
        else:
            for flag, expected in FAIL_CLOSED_SELECTION_FLAGS.items():
                if flags.get(flag) is not expected:
                    errors.append(f"M030_CONTINUITY_SELECTION_FLAGS: {flag} must be {expected!r}")

    stages = audit.get("stages")
    if not isinstance(stages, list):
        errors.append("M030_CONTINUITY_STAGES: stages must be a list")
        stages = []
    stage_ids: set[str] = set()
    for index, row in enumerate(stages):
        if not isinstance(row, dict):
            errors.append(f"M030_CONTINUITY_STAGE_ROW: stages[{index}] must be an object")
            continue
        stage_id = row.get("stage_id")
        if not isinstance(stage_id, str) or not stage_id.strip():
            errors.append(f"M030_CONTINUITY_STAGE_ID: stages[{index}] missing stage_id")
            stage_id = f"stages[{index}]"
        else:
            if stage_id in stage_ids:
                errors.append(f"M030_CONTINUITY_STAGE_ID: duplicate stage_id {stage_id}")
            stage_ids.add(stage_id)
        for field in ("owner_modules", "inputs", "outputs", "verifiers", "observable_surface"):
            if not _has_nonempty_list(row.get(field)):
                errors.append(f"M030_CONTINUITY_STAGE_FIELD: {stage_id} missing non-empty {field}")
        if not isinstance(row.get("claim_state"), str) or not row["claim_state"].strip():
            errors.append(f"M030_CONTINUITY_STAGE_FIELD: {stage_id} missing claim_state")
    missing_stages = [stage for stage in EXPECTED_STAGE_ORDER if stage not in stage_ids]
    if missing_stages:
        errors.append(f"M030_CONTINUITY_STAGES: missing required stages {missing_stages}")

    edges = audit.get("continuity_edges")
    if not isinstance(edges, list):
        errors.append("M030_CONTINUITY_EDGES: continuity_edges must be a list")
        edges = []
    edge_ids: set[str] = set()
    edges_by_id: dict[str, dict[str, Any]] = {}
    for index, row in enumerate(edges):
        if not isinstance(row, dict):
            errors.append(f"M030_CONTINUITY_EDGE_ROW: continuity_edges[{index}] must be an object")
            continue
        edge_id = row.get("edge_id")
        if not isinstance(edge_id, str) or not edge_id.strip():
            errors.append(f"M030_CONTINUITY_EDGE_ID: continuity_edges[{index}] missing edge_id")
            edge_id = f"continuity_edges[{index}]"
        else:
            if edge_id in edge_ids:
                errors.append(f"M030_CONTINUITY_EDGE_ID: duplicate edge_id {edge_id}")
            edge_ids.add(edge_id)
            edges_by_id[edge_id] = row
        from_stage = row.get("from_stage")
        to_stage = row.get("to_stage")
        if from_stage not in stage_ids:
            errors.append(f"M030_CONTINUITY_EDGE_STAGE: {edge_id} from_stage {from_stage!r} missing stage row")
        if to_stage not in stage_ids:
            errors.append(f"M030_CONTINUITY_EDGE_STAGE: {edge_id} to_stage {to_stage!r} missing stage row")
        if not isinstance(row.get("continuity_state"), str) or not row["continuity_state"].strip():
            errors.append(f"M030_CONTINUITY_EDGE_FIELD: {edge_id} missing continuity_state")
        for field in sorted(REQUIRED_EDGE_FIELDS):
            if not _has_nonempty_list(row.get(field)):
                errors.append(f"M030_CONTINUITY_EDGE_FIELD: {edge_id} missing non-empty {field}")
        if len(_as_nonempty_str_list(row.get("failure_modes"))) < 2:
            errors.append(f"M030_CONTINUITY_EDGE_FAILURE_MODES: {edge_id} must describe at least two failure modes")

    missing_edges = sorted(set(EXPECTED_EDGES) - edge_ids)
    if missing_edges:
        errors.append(f"M030_CONTINUITY_EDGES: missing required edges {missing_edges}")
    for edge_id, (expected_from, expected_to) in EXPECTED_EDGES.items():
        row = edges_by_id.get(edge_id)
        if not row:
            continue
        if row.get("from_stage") != expected_from or row.get("to_stage") != expected_to:
            errors.append(
                "M030_CONTINUITY_EDGE_ORDER: "
                f"{edge_id} expected {expected_from}->{expected_to}, got {row.get('from_stage')}->{row.get('to_stage')}"
            )

    graph_stage = [row for row in stages if isinstance(row, dict) and row.get("stage_id") == "graph_import_boundary"]
    graph_edges = [row for row in edges if isinstance(row, dict) and "graph_import" in str(row.get("edge_id", ""))]
    graph_payload = {"stage": graph_stage, "edges": graph_edges, "diagnostic_contract": audit.get("diagnostic_contract")}
    if not _contains_fail_closed_boundary(graph_payload):
        errors.append("M030_CONTINUITY_GRAPH_IMPORT_BOUNDARY: graph/import readiness is not visibly fail-closed")

    breakpoints = audit.get("known_breakpoints")
    if not isinstance(breakpoints, list):
        errors.append("M030_CONTINUITY_BREAKPOINTS: known_breakpoints must be a list")
        breakpoints = []
    breakpoint_ids = {row.get("breakpoint_id") for row in breakpoints if isinstance(row, dict)}
    missing_breakpoints = sorted(REQUIRED_BREAKPOINTS - {item for item in breakpoint_ids if isinstance(item, str)})
    if missing_breakpoints:
        errors.append(f"M030_CONTINUITY_BREAKPOINTS: missing required breakpoints {missing_breakpoints}")
    for row in breakpoints:
        if not isinstance(row, dict):
            continue
        breakpoint_id = row.get("breakpoint_id", "<unknown>")
        for field in ("stage", "status", "evidence", "required_next_action"):
            if not isinstance(row.get(field), str) or not row[field].strip():
                errors.append(f"M030_CONTINUITY_BREAKPOINT_FIELD: {breakpoint_id} missing {field}")

    diagnostic_contract = audit.get("diagnostic_contract")
    if not isinstance(diagnostic_contract, dict):
        errors.append("M030_CONTINUITY_DIAGNOSTIC_CONTRACT: diagnostic_contract must be an object")
    else:
        required_future_fields = set(_as_nonempty_str_list(diagnostic_contract.get("edge_fields_required_for_future_updates")))
        if required_future_fields != ({"edge_id", "from_stage", "to_stage", "continuity_state"} | REQUIRED_EDGE_FIELDS):
            errors.append("M030_CONTINUITY_DIAGNOSTIC_CONTRACT: edge_fields_required_for_future_updates is incomplete")
        fail_closed_flags = _as_nonempty_str_list(diagnostic_contract.get("fail_closed_booleans_to_preserve_until_verified"))
        for flag in FAIL_CLOSED_SELECTION_FLAGS:
            if not any(flag in item for item in fail_closed_flags):
                errors.append(f"M030_CONTINUITY_DIAGNOSTIC_CONTRACT: missing fail-closed flag {flag}")

    return errors


def validate_report(report_path: Path, audit: dict[str, Any]) -> list[str]:
    text = _load_text(report_path)
    lower_text = text.lower()
    errors: list[str] = []
    for phrase in REQUIRED_REPORT_PHRASES:
        if phrase not in text:
            errors.append(f"M030_CONTINUITY_REPORT_PHRASE: report missing {phrase!r}")
    for stage_id in EXPECTED_STAGE_ORDER:
        if stage_id not in text and stage_id.replace("_", " ") not in lower_text:
            errors.append(f"M030_CONTINUITY_REPORT_STAGE: report missing stage {stage_id}")
    for breakpoint_id in REQUIRED_BREAKPOINTS:
        if breakpoint_id not in text and breakpoint_id.split("_", 1)[0] not in text:
            errors.append(f"M030_CONTINUITY_REPORT_BREAKPOINT: report missing breakpoint {breakpoint_id}")
    if not _contains_fail_closed_boundary({"report": text, "audit": audit.get("diagnostic_contract")}):
        errors.append("M030_CONTINUITY_REPORT_FAIL_CLOSED: report missing fail-closed graph/import boundary language")
    return errors


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--audit", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--validate-only", action="store_true", help="Validate existing local audit/report artifacts only.")
    args = parser.parse_args(argv)
    if not args.validate_only:
        parser.error("only --validate-only is supported; this verifier must not fetch, replay, or write")
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    errors: list[str] = []
    audit: dict[str, Any] = {}
    try:
        audit = _load_json(args.audit)
        root = Path.cwd()
        errors.extend(validate_audit(audit, root=root))
        errors.extend(validate_report(args.report, audit))
    except (OSError, ValueError) as exc:
        errors.append(f"M030_CONTINUITY_IO: {exc}")

    if errors:
        sys.stderr.write("M030 process continuity audit validation failed:\n")
        for error in errors:
            sys.stderr.write(f"- {error}\n")
        return 1

    sys.stdout.write(
        "M030 process continuity audit validation passed: "
        f"stages={len(EXPECTED_STAGE_ORDER)} edges={len(EXPECTED_EDGES)} "
        "edge_fields_complete=true source_artifacts_present=true "
        "graph_import_boundary_fail_closed=true.\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
