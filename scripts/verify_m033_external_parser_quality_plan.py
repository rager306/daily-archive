#!/usr/bin/env python3
"""Validate M033 S06 bounded external parser quality plan artifacts."""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Any

FALSE_FLAG_KEYS = (
    "graph_import_allowed",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
    "trusted_kg_import_allowed",
    "graph_write_attempted",
)
EXPECTED_METRIC_CATEGORIES = {
    "grobid_tei_bibliography_citation_quality",
    "opendataloader_layout_ocr_table_coordinate_quality",
    "adaptix_adapter_contract_coverage",
    "tree_pageindex_card_provenance_schema_fit",
    "source_span_anchoring_and_staleness",
    "low_quality_and_refusal_preservation",
    "review_packet_and_graph_readiness_boundary",
}
EXPECTED_DIAGNOSTICS = {
    "missing_local_source",
    "unsafe_or_stale_source_hash",
    "backend_unhealthy",
    "model_cache_missing_no_network",
    "tei_parse_failed",
    "bibliography_quality_below_gate",
    "layout_quality_below_gate",
    "table_fidelity_below_gate",
    "ocr_quality_below_gate",
    "adaptix_mapping_failed",
    "invalid_evidence_path",
    "low_quality_source",
    "review_packet_incomplete",
    "graph_readiness_postcheck_failed",
}


def load_json(path: Path, failures: list[dict[str, Any]]) -> dict[str, Any]:
    if not path.exists() or path.stat().st_size == 0:
        failures.append({"code": "missing_json", "path": str(path)})
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        failures.append({"code": "invalid_json", "path": str(path), "error": str(exc)})
        return {}


def require_text(path: Path, failures: list[dict[str, Any]]) -> str:
    if not path.exists() or path.stat().st_size == 0:
        failures.append({"code": "missing_text", "path": str(path)})
        return ""
    return path.read_text(encoding="utf-8", errors="replace")


def require_false_flags(
    owner: str, flags: dict[str, Any] | None, failures: list[dict[str, Any]]
) -> None:
    if not isinstance(flags, dict):
        failures.append({"code": "missing_safety_flags", "owner": owner})
        return
    for key in FALSE_FLAG_KEYS:
        if flags.get(key) is not False:
            failures.append(
                {"code": "unsafe_flag", "owner": owner, "flag": key, "value": flags.get(key)}
            )


def validate_scope(root: Path, failures: list[dict[str, Any]]) -> None:
    scope = load_json(root / "future-probe-scope.json", failures)
    text = require_text(root / "future-probe-scope.md", failures)
    if scope:
        require_false_flags("future_probe_scope", scope.get("safety_flags"), failures)
        if (
            scope.get("derived_from_recommendation")
            != "recommended-bounded-combined-sidecar-architecture"
        ):
            failures.append(
                {
                    "code": "unexpected_scope_source",
                    "value": scope.get("derived_from_recommendation"),
                }
            )
        if scope.get("not_executed_in_M033") is not True:
            failures.append({"code": "future_scope_executed_in_m033"})
        if len(scope.get("corpus_classes", [])) < 6:
            failures.append({"code": "too_few_corpus_classes"})
        excluded = set(scope.get("excluded_production_actions", []))
        for needle in (
            "production parser integration",
            "graph import",
            "LadybugDB write",
            "positive import eligibility claim",
        ):
            if needle not in excluded:
                failures.append({"code": "missing_excluded_action", "needle": needle})
    for needle in (
        "no-network",
        "model/backend cache",
        "graph_import_allowed=false",
        "production parser integration",
    ):
        if needle not in text:
            failures.append({"code": "missing_scope_text", "needle": needle})


def validate_metrics(root: Path, failures: list[dict[str, Any]]) -> None:
    metrics = load_json(root / "quality-metrics-and-gates.json", failures)
    text = require_text(root / "quality-metrics-and-gates.md", failures)
    if metrics:
        require_false_flags("quality_metrics", metrics.get("safety_flags"), failures)
        categories = {item.get("category") for item in metrics.get("metric_categories", [])}
        missing = EXPECTED_METRIC_CATEGORIES - categories
        if missing:
            failures.append({"code": "missing_metric_categories", "missing": sorted(missing)})
        rules = "\n".join(metrics.get("global_acceptance_rules", []))
        for needle in ("no graph import", "no secrets", "graph-readiness review post-check"):
            if needle not in rules:
                failures.append({"code": "missing_global_rule", "needle": needle})
        if "--require-completed-review" not in metrics.get("review_post_check_command", ""):
            failures.append({"code": "missing_review_postcheck_command"})
    for needle in (
        "GROBID",
        "OpenDataLoader",
        "Adaptix",
        "low_quality_source",
        "graph-readiness review post-check",
    ):
        if needle not in text:
            failures.append({"code": "missing_metrics_text", "needle": needle})


def validate_contracts(root: Path, failures: list[dict[str, Any]]) -> None:
    contracts = load_json(root / "artifact-contracts-and-diagnostics.json", failures)
    text = require_text(root / "artifact-contracts-and-diagnostics.md", failures)
    rollback_text = require_text(root / "adoption-and-rollback-criteria.md", failures)
    if contracts:
        require_false_flags("artifact_contracts", contracts.get("safety_flags"), failures)
        logging_rules = set(contracts.get("logging_rules", []))
        if "No secrets in artifacts or logs" not in logging_rules:
            failures.append({"code": "missing_no_secret_rule"})
        if "No raw article bodies in diagnostics or aggregate summaries" not in logging_rules:
            failures.append({"code": "missing_no_raw_body_rule"})
        diagnostics = {item.get("code") for item in contracts.get("diagnostic_taxonomy", [])}
        missing = EXPECTED_DIAGNOSTICS - diagnostics
        if missing:
            failures.append({"code": "missing_diagnostic_codes", "missing": sorted(missing)})
        rehearsal = contracts.get("no_write_import_rehearsal", {})
        expected_zero = ("accepted_count", "import_eligible_count", "ladybugdb_write_attempts")
        for key in expected_zero:
            if rehearsal.get(key) != 0:
                failures.append(
                    {
                        "code": "nonzero_no_write_rehearsal",
                        "field": key,
                        "value": rehearsal.get(key),
                    }
                )
        if rehearsal.get("graph_import_allowed") is not False:
            failures.append({"code": "rehearsal_allows_graph_import"})
    for needle in (
        "No secrets",
        "No raw article bodies",
        "low_quality_source",
        "graph_readiness_postcheck_failed",
    ):
        if needle not in text:
            failures.append({"code": "missing_contracts_text", "needle": needle})
    for needle in (
        "adoption_decision_allowed_by_M033: `false`",
        "graph import",
        "LadybugDB writes",
        "import_eligible=false",
    ):
        if needle not in rollback_text:
            failures.append({"code": "missing_rollback_text", "needle": needle})


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def write_report(path: Path, closeout: dict[str, Any]) -> None:
    lines = [
        "# M033 S06 External Parser Quality Plan Closeout",
        "",
        f"- status: `{closeout['status']}`",
        f"- failure_count: `{len(closeout['failures'])}`",
        f"- verdict: `{closeout['verdict']}`",
        "- future_probe_only: `true`",
        "- production_integration_authorized: `false`",
        "- graph_import_allowed=false",
        "- ladybugdb_written=false",
        "- production_import_attempted=false",
        "- import_eligible=false",
        "",
    ]
    if closeout["failures"]:
        lines.extend(["## Failures", ""])
        lines.extend(f"- `{failure['code']}` {failure}" for failure in closeout["failures"])
    else:
        lines.extend(
            [
                "## Result",
                "",
                "The bounded external parser quality plan is internally consistent and fail-closed.",
                "It defines a future probe only and authorizes no production integration, graph import, LadybugDB write, or import eligibility claim.",
            ]
        )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--plan-dir", required=True, type=Path)
    args = parser.parse_args()
    root = args.plan_dir
    failures: list[dict[str, Any]] = []
    if not root.exists():
        failures.append({"code": "missing_plan_dir", "path": str(root)})
    else:
        validate_scope(root, failures)
        validate_metrics(root, failures)
        validate_contracts(root, failures)
        events = root / "quality-plan-events.jsonl"
        if not events.exists() or events.stat().st_size == 0:
            failures.append({"code": "missing_events", "path": str(events)})
    status = "passed" if not failures else "failed"
    closeout = {
        "schema_version": "m033.external_parser_quality.closeout_summary.v1",
        "generated_at_epoch": int(time.time()),
        "status": status,
        "verdict": "bounded-future-quality-plan-ready",
        "future_probe_only": True,
        "production_integration_authorized": False,
        "dependency_adoption_authorized": False,
        "failures": failures,
        "safety_flags": dict.fromkeys(FALSE_FLAG_KEYS, False),
    }
    write_json(root / "quality-plan-closeout-summary.json", closeout)
    write_report(root / "quality-plan-closeout-report.md", closeout)
    sys.stdout.write(
        json.dumps(
            {
                "status": status,
                "failure_count": len(failures),
                "verdict": closeout["verdict"],
            },
            indent=2,
        )
        + "\n"
    )
    return 0 if status == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
