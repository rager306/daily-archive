#!/usr/bin/env python3
"""Validate M033 S05 combined parser architecture recommendation artifacts."""

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
EXPECTED_VERDICTS = {
    "baseline-established",
    "grobid-scholarly-sidecar-candidate",
    "hybrid-sidecar-candidate",
    "adaptix-adapter-candidate",
    "pattern-source-not-dependency",
}
EXPECTED_COMPONENTS = {
    "GROBID",
    "OpenDataLoader-style extraction",
    "Adaptix",
    "quant-mind patterns",
    "daily-archive",
}
EXPECTED_RISK_CATEGORIES = {
    "grobid_runtime_and_accuracy",
    "opendataloader_hybrid_backend_and_cache",
    "layout_table_ocr_fidelity",
    "source_span_and_coordinate_anchoring",
    "adaptix_structural_vs_semantic_boundary",
    "quantmind_pattern_reimplementation",
    "graph_readiness_and_no_write_import_boundary",
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


def validate_matrix(root: Path, failures: list[dict[str, Any]]) -> None:
    matrix = load_json(root / "synthesis-evidence-matrix.json", failures)
    text = require_text(root / "synthesis-evidence-matrix.md", failures)
    if matrix:
        require_false_flags("synthesis_matrix", matrix.get("safety_flags"), failures)
        entries = matrix.get("entries", [])
        verdicts = {entry.get("verdict") for entry in entries}
        missing = EXPECTED_VERDICTS - verdicts
        if missing:
            failures.append({"code": "missing_expected_verdicts", "missing": sorted(missing)})
        slices = {entry.get("slice") for entry in entries}
        if slices != {"S01", "S02", "S03", "S04", "S07"}:
            failures.append({"code": "unexpected_matrix_slices", "value": sorted(slices)})
        for entry in entries:
            require_false_flags(
                f"matrix_entry_{entry.get('slice')}", entry.get("safety_flags"), failures
            )
    for needle in (
        "GROBID",
        "OpenDataLoader",
        "Adaptix",
        "quant-mind",
        "daily-archive",
        "candidate_only",
    ):
        if needle not in text:
            failures.append({"code": "missing_matrix_text", "needle": needle})


def validate_recommendation(root: Path, failures: list[dict[str, Any]]) -> None:
    recommendation = load_json(root / "combined-parser-recommendation.json", failures)
    text = require_text(root / "combined-parser-recommendation.md", failures)
    if recommendation:
        require_false_flags("recommendation", recommendation.get("safety_flags"), failures)
        if recommendation.get("verdict") != "recommended-bounded-combined-sidecar-architecture":
            failures.append(
                {
                    "code": "unexpected_recommendation_verdict",
                    "value": recommendation.get("verdict"),
                }
            )
        for key in ("candidate_only",):
            if recommendation.get(key) is not True:
                failures.append({"code": "expected_true", "owner": "recommendation", "field": key})
        for key in ("production_adoption_authorized", "runtime_dependency_adoption_authorized"):
            if recommendation.get(key) is not False:
                failures.append({"code": "expected_false", "owner": "recommendation", "field": key})
        components = {
            item.get("component") for item in recommendation.get("component_responsibilities", [])
        }
        missing = EXPECTED_COMPONENTS - components
        if missing:
            failures.append({"code": "missing_components", "missing": sorted(missing)})
        if len(recommendation.get("rejected_alternatives", [])) < 5:
            failures.append({"code": "too_few_rejected_alternatives"})
    for needle in (
        "recommended-bounded-combined-sidecar-architecture",
        "production_adoption_authorized: `false`",
        "runtime_dependency_adoption_authorized: `false`",
        "Parser outputs are candidate evidence only",
        "direct parser-to-LadybugDB import",
    ):
        if needle not in text:
            failures.append({"code": "missing_recommendation_text", "needle": needle})


def validate_gates(root: Path, failures: list[dict[str, Any]]) -> None:
    gates = load_json(root / "complexity-and-validation-gates.json", failures)
    text = require_text(root / "complexity-and-validation-gates.md", failures)
    if gates:
        require_false_flags("complexity_gates", gates.get("safety_flags"), failures)
        categories = {item.get("category") for item in gates.get("risk_categories", [])}
        missing = EXPECTED_RISK_CATEGORIES - categories
        if missing:
            failures.append({"code": "missing_risk_categories", "missing": sorted(missing)})
        gate_text = "\n".join(gates.get("validation_gates", []))
        for needle in ("graph-readiness review post-check", "no-write import rehearsal"):
            if needle not in gate_text:
                failures.append({"code": "missing_validation_gate", "needle": needle})
    for needle in (
        "graph_readiness_and_no_write_import_boundary",
        "S06 must turn these risks",
        "production integration",
    ):
        if needle not in text:
            failures.append({"code": "missing_gates_text", "needle": needle})


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def write_report(path: Path, closeout: dict[str, Any]) -> None:
    lines = [
        "# M033 S05 Combined Parser Architecture Closeout",
        "",
        f"- status: `{closeout['status']}`",
        f"- failure_count: `{len(closeout['failures'])}`",
        f"- verdict: `{closeout['verdict']}`",
        "- candidate_only: `true`",
        "- production_adoption_authorized: `false`",
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
                "The bounded combined sidecar architecture recommendation is internally consistent and fail-closed.",
                "It authorizes no production adoption, graph import, LadybugDB write, or import eligibility claim.",
            ]
        )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--architecture-dir", required=True, type=Path)
    args = parser.parse_args()
    root = args.architecture_dir
    failures: list[dict[str, Any]] = []
    if not root.exists():
        failures.append({"code": "missing_architecture_dir", "path": str(root)})
    else:
        validate_matrix(root, failures)
        validate_recommendation(root, failures)
        validate_gates(root, failures)
        events = root / "synthesis-events.jsonl"
        if not events.exists() or events.stat().st_size == 0:
            failures.append({"code": "missing_events", "path": str(events)})
    status = "passed" if not failures else "failed"
    closeout = {
        "schema_version": "m033.combined_parser.closeout_summary.v1",
        "generated_at_epoch": int(time.time()),
        "status": status,
        "verdict": "recommended-bounded-combined-sidecar-architecture",
        "candidate_only": True,
        "production_adoption_authorized": False,
        "runtime_dependency_adoption_authorized": False,
        "failures": failures,
        "safety_flags": dict.fromkeys(FALSE_FLAG_KEYS, False),
    }
    write_json(root / "combined-architecture-closeout-summary.json", closeout)
    write_report(root / "combined-architecture-closeout-report.md", closeout)
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
