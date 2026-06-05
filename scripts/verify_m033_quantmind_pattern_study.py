#!/usr/bin/env python3
"""Validate M033 S04 quant-mind pattern-study artifacts.

The verifier is intentionally fail-closed: quant-mind can only be a static
architecture pattern source in M033. Live runtime, graph/import/write claims,
or dependency adoption are closeout failures.
"""

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
)
OPTIONAL_FALSE_FLAG_KEYS = (
    "trusted_kg_import_allowed",
    "graph_write_attempted",
    "model_call_required_for_pattern_study",
    "network_required_for_pattern_study",
)


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


def require_false_flags(owner: str, flags: dict[str, Any] | None, failures: list[dict[str, Any]]) -> None:
    if not isinstance(flags, dict):
        failures.append({"code": "missing_safety_flags", "owner": owner})
        return
    for key in FALSE_FLAG_KEYS:
        if flags.get(key) is not False:
            failures.append({"code": "unsafe_flag", "owner": owner, "flag": key, "value": flags.get(key)})
    for key in OPTIONAL_FALSE_FLAG_KEYS:
        if key in flags and flags.get(key) is not False:
            failures.append({"code": "unsafe_optional_flag", "owner": owner, "flag": key, "value": flags.get(key)})


def validate_requirements(study_dir: Path, failures: list[dict[str, Any]]) -> None:
    summary = load_json(study_dir / "quantmind-requirements-summary.json", failures)
    text = require_text(study_dir / "quantmind-runtime-decision.md", failures)
    if summary:
        require_false_flags("requirements_summary", summary.get("safety_flags"), failures)
        runtime = summary.get("runtime_requirements", {})
        if runtime.get("python_requires") != ">=3.10":
            failures.append({"code": "missing_python_requirement", "value": runtime.get("python_requires")})
        if runtime.get("docker_or_compose_found") is not False or runtime.get("container_required") is not False:
            failures.append({"code": "unexpected_container_requirement"})
        deps = set(summary.get("core_dependencies", []))
        if "openai-agents>=0.14" not in deps:
            failures.append({"code": "missing_openai_agents_dependency"})
        env = summary.get("env_requirements", {})
        if "OPENAI_API_KEY" not in env:
            failures.append({"code": "missing_openai_key_requirement"})
        decision = summary.get("runtime_decision", {})
        if decision.get("decision") != "do-not-run-live-quantmind-runtime-in-M033-S04":
            failures.append({"code": "unexpected_runtime_decision", "value": decision.get("decision")})
        if "paper_flow" not in decision.get("do_not_run", []):
            failures.append({"code": "paper_flow_not_forbidden"})
    for needle in (
        "Do not run live `quantmind.paper_flow`",
        "OPENAI_API_KEY",
        "graph_import_allowed=false",
        "model_call_required_for_pattern_study=false",
    ):
        if needle not in text:
            failures.append({"code": "missing_runtime_decision_text", "needle": needle})


def validate_implemented_vs_vision(study_dir: Path, failures: list[dict[str, Any]]) -> None:
    payload = load_json(study_dir / "quantmind-implemented-vs-vision.json", failures)
    text = require_text(study_dir / "quantmind-implemented-vs-vision.md", failures)
    if payload:
        require_false_flags("implemented_vs_vision", payload.get("safety_flags"), failures)
        implemented = {item.get("name"): item.get("status") for item in payload.get("implemented_or_usable_patterns", [])}
        not_ready = {item.get("name"): item.get("status") for item in payload.get("not_ready_or_aspirational", [])}
        expected_impl = {
            "TreeKnowledge": "implemented-pattern",
            "Paper and PaperKnowledgeCard": "implemented-pattern",
            "BaseKnowledge provenance": "implemented-pattern",
        }
        for name, expected in expected_impl.items():
            if implemented.get(name) != expected:
                failures.append({"code": "implemented_pattern_missing", "name": name, "value": implemented.get(name)})
        expected_not_ready = {
            "GraphKnowledge": "placeholder-not-implemented",
            "storage layer": "missing-from-package",
            "retrieval API / RAG runtime": "missing-from-package",
            "memory / mind": "placeholder-or-roadmap",
        }
        for name, expected in expected_not_ready.items():
            if not_ready.get(name) != expected:
                failures.append({"code": "not_ready_boundary_missing", "name": name, "value": not_ready.get(name)})
        classification = payload.get("classification", {})
        if classification.get("as_runtime_dependency_for_M033") != "not_recommended":
            failures.append({"code": "runtime_dependency_not_rejected"})
    for needle in ("GraphKnowledge", "storage layer", "retrieval API", "TreeKnowledge", "PaperKnowledgeCard"):
        if needle not in text:
            failures.append({"code": "missing_vision_report_text", "needle": needle})


def validate_pattern_map(study_dir: Path, failures: list[dict[str, Any]]) -> None:
    pattern_map = load_json(study_dir / "quantmind-daily-archive-pattern-map.json", failures)
    verdict = load_json(study_dir / "quantmind-pattern-verdict.json", failures)
    text = require_text(study_dir / "quantmind-daily-archive-pattern-map.md", failures)
    if pattern_map:
        require_false_flags("pattern_map", pattern_map.get("safety_flags"), failures)
        if pattern_map.get("classification") != "pattern-source-not-dependency":
            failures.append({"code": "unexpected_pattern_classification", "value": pattern_map.get("classification")})
        names = {item.get("quantmind_pattern") for item in pattern_map.get("patterns_to_adopt", [])}
        for name in (
            "TreeKnowledge / TreeNode",
            "Paper + PaperKnowledgeCard split",
            "SourceRef / Citation / ExtractionRef",
            "preprocess.fetch + preprocess.format + flow separation",
            "batch_run bounded concurrency",
            "magic resolver schema introspection guardrails",
        ):
            if name not in names:
                failures.append({"code": "missing_adopted_pattern", "name": name})
    if verdict:
        require_false_flags("pattern_verdict", verdict.get("safety_flags"), failures)
        if verdict.get("verdict") != "pattern-source-not-dependency":
            failures.append({"code": "unexpected_verdict", "value": verdict.get("verdict")})
        if verdict.get("candidate_only") is not True:
            failures.append({"code": "verdict_not_candidate_only"})
        forbidden_roles = set(verdict.get("not_recommended_for", []))
        if "M033 runtime dependency" not in forbidden_roles:
            failures.append({"code": "runtime_dependency_not_forbidden_in_verdict"})
    for needle in (
        "pattern-source-not-dependency",
        "TreeKnowledge",
        "PaperKnowledgeCard",
        "SourceRef",
        "GROBID",
        "OpenDataLoader",
        "Adaptix",
        "graph_import_allowed=false",
        "ladybugdb_written=false",
    ):
        if needle not in text:
            failures.append({"code": "missing_pattern_report_text", "needle": needle})


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def write_report(path: Path, closeout: dict[str, Any]) -> None:
    lines = [
        "# M033 S04 QuantMind Pattern Study Closeout",
        "",
        f"- status: `{closeout['status']}`",
        f"- failure_count: `{len(closeout['failures'])}`",
        f"- verdict: `{closeout['verdict']}`",
        "- candidate_only: `true`",
        "- runtime_dependency: `false`",
        "- live_quantmind_runtime_executed: `false`",
        "- graph_import_allowed=false",
        "- ladybugdb_written=false",
        "- production_import_attempted=false",
        "- import_eligible=false",
        "",
    ]
    if closeout["failures"]:
        lines.append("## Failures")
        lines.append("")
        for failure in closeout["failures"]:
            lines.append(f"- `{failure['code']}` {failure}")
    else:
        lines.extend(
            [
                "## Result",
                "",
                "quant-mind is validated as a static architecture pattern source, not a production dependency.",
                "No OpenAI/API/network runtime execution is required or claimed for S04.",
            ]
        )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--study-dir", required=True, type=Path)
    args = parser.parse_args()
    study_dir = args.study_dir
    failures: list[dict[str, Any]] = []
    if not study_dir.exists():
        failures.append({"code": "missing_study_dir", "path": str(study_dir)})
    else:
        validate_requirements(study_dir, failures)
        validate_implemented_vs_vision(study_dir, failures)
        validate_pattern_map(study_dir, failures)
        events = study_dir / "quantmind-pattern-events.jsonl"
        if not events.exists() or events.stat().st_size == 0:
            failures.append({"code": "missing_events", "path": str(events)})

    status = "passed" if not failures else "failed"
    closeout = {
        "schema_version": "m033.quantmind.closeout-summary.v1",
        "created_at_epoch": int(time.time()),
        "status": status,
        "verdict": "pattern-source-not-dependency",
        "candidate_only": True,
        "runtime_dependency": False,
        "live_quantmind_runtime_executed": False,
        "failures": failures,
        "safety_flags": dict.fromkeys(FALSE_FLAG_KEYS + OPTIONAL_FALSE_FLAG_KEYS, False),
    }
    write_json(study_dir / "quantmind-closeout-summary.json", closeout)
    write_report(study_dir / "quantmind-closeout-report.md", closeout)
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
