#!/usr/bin/env python3
"""Generate the M198 metadata-only readiness report."""

from __future__ import annotations

import argparse
import json
from collections import Counter
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

INDEX_SCHEMA_VERSION = "m198.readiness_evidence_index.v1"
DIAGNOSTICS_SCHEMA_VERSION = "m198.operator_diagnostics.v1"
REPORT_SCHEMA_VERSION = "m198.readiness_report.v1"


def _load_json_object(path: Path, expected_schema: str) -> dict[str, Any]:
    loaded = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(loaded, dict):
        raise ValueError(f"{path} must contain a JSON object")
    if loaded.get("schema_version") != expected_schema:
        raise ValueError(f"expected {expected_schema}, got {loaded.get('schema_version')!r}")
    return loaded


def _payload_policy_ok(index: dict[str, Any], diagnostics: dict[str, Any]) -> bool:
    if index.get("metadata_only") is not True:
        return False
    if diagnostics.get("metadata_only") is not True:
        return False
    if diagnostics.get("payload_policy_confirmed") is not True:
        return False
    policy = index.get("payload_policy") or {}
    return all(
        policy.get(key) is False
        for key in (
            "stores_payload_text",
            "stores_embeddings",
            "stores_vectors",
            "stores_credentials",
            "stores_queue_database_bytes",
        )
    )


def _drift_summary(index: dict[str, Any]) -> dict[str, int]:
    counter: Counter[str] = Counter()
    for entry in index.get("entries") or []:
        if isinstance(entry, dict):
            drift_class = entry.get("drift_class")
            if isinstance(drift_class, str) and drift_class:
                counter[drift_class] += 1
    return dict(sorted(counter.items()))


def _source_coverage(index: dict[str, Any], diagnostics: dict[str, Any]) -> dict[str, Any]:
    diagnostic_coverage = diagnostics.get("source_coverage") or {}
    required = list(index.get("required_source_kinds") or diagnostic_coverage.get("required_source_kinds") or [])
    observed = list(index.get("observed_source_kinds") or diagnostic_coverage.get("observed_source_kinds") or [])
    missing = list(index.get("missing_source_kinds") or diagnostic_coverage.get("missing_source_kinds") or [])
    return {
        "required_count": len(required),
        "observed_count": len(observed),
        "missing_count": len(missing),
        "required_source_kinds": required,
        "observed_source_kinds": observed,
        "missing_source_kinds": missing,
    }


def _disagreements(index: dict[str, Any], diagnostics: dict[str, Any], coverage: dict[str, Any]) -> list[str]:
    findings: list[str] = []
    index_status = index.get("status")
    diagnostics_status = diagnostics.get("index_status")
    if diagnostics_status is not None and diagnostics_status != index_status:
        findings.append(f"diagnostics index_status {diagnostics_status!r} disagrees with index status {index_status!r}")
    diagnostics_coverage = diagnostics.get("source_coverage") or {}
    if diagnostics_coverage.get("missing_source_kinds") not in (None, coverage["missing_source_kinds"]):
        findings.append("diagnostics missing_source_kinds disagrees with index missing_source_kinds")
    if diagnostics.get("verdict") == "ready" and (index.get("blockers") or index.get("warnings") or coverage["missing_source_kinds"]):
        findings.append("diagnostics ready verdict disagrees with index warnings, blockers, or missing sources")
    return findings


def build_report(index: dict[str, Any], diagnostics: dict[str, Any]) -> dict[str, Any]:
    coverage = _source_coverage(index, diagnostics)
    warnings = sorted({str(item) for item in (index.get("warnings") or []) + (diagnostics.get("warnings") or [])})
    blockers = sorted({str(item) for item in (index.get("blockers") or []) + (diagnostics.get("blockers") or [])})
    disagreements = _disagreements(index, diagnostics, coverage)
    payload_ok = _payload_policy_ok(index, diagnostics)
    if coverage["missing_source_kinds"]:
        blockers.append("missing source kinds: " + ", ".join(coverage["missing_source_kinds"]))
    if disagreements:
        blockers.extend(disagreements)
    if not payload_ok:
        blockers.append("metadata-only payload policy is not confirmed")

    blockers = sorted(set(blockers))
    diagnostics_verdict = diagnostics.get("verdict")
    if blockers or index.get("status") == "fail" or diagnostics_verdict == "blocked":
        verdict = "blocked"
    elif warnings or diagnostics_verdict == "needs_attention":
        verdict = "needs_attention"
    else:
        verdict = "ready"

    return {
        "schema_version": REPORT_SCHEMA_VERSION,
        "generated_at": datetime.now(UTC).isoformat(),
        "verdict": verdict,
        "ready": verdict == "ready",
        "index_status": index.get("status"),
        "diagnostics_verdict": diagnostics_verdict,
        "source_coverage": coverage,
        "entry_count": index.get("entry_count"),
        "drift_summary": _drift_summary(index),
        "warnings": warnings,
        "blockers": blockers,
        "disagreements": disagreements,
        "blocked_transitions": sorted(set((index.get("non_goal_coverage") or []) + (diagnostics.get("blocked_transitions") or []))),
        "payload_policy_confirmed": payload_ok,
        "metadata_only": index.get("metadata_only") is True and diagnostics.get("metadata_only") is True,
        "next_actions": _next_actions(verdict, blockers, warnings),
        "downstream_handoff": [
            "S11 no-write governance ratchets",
            "S13 realistic readiness rehearsal",
            "S16 end-to-end validation package",
        ],
    }


def _next_actions(verdict: str, blockers: list[str], warnings: list[str]) -> list[str]:
    if verdict == "ready":
        return [
            "Proceed to S11 no-write governance ratchets.",
            "Use this report as the S13 rehearsal target without enabling graph writes or imports.",
        ]
    if verdict == "needs_attention":
        return [
            "Review warnings before adding S11 governance ratchets.",
            "Keep readiness promotion blocked from production import until governance ratchets pass.",
        ]
    actions = ["Stop readiness promotion until report blockers are resolved."]
    if any("payload" in blocker or "metadata-only" in blocker for blocker in blockers):
        actions.append("Fix metadata-only payload policy before S11 or S13 consumption.")
    if any("missing source" in blocker for blocker in blockers):
        actions.append("Regenerate missing S03-S07 producer or classifier evidence before reporting readiness.")
    if warnings:
        actions.append("Re-check warnings after blockers clear.")
    return actions


def render_markdown(report: dict[str, Any]) -> str:
    lines = [
        "# M198 Readiness Report",
        "",
        f"- Verdict: `{report['verdict']}`",
        f"- Ready: `{str(report['ready']).lower()}`",
        f"- Index status: `{report['index_status']}`",
        f"- Diagnostics verdict: `{report['diagnostics_verdict']}`",
        f"- Metadata only: `{str(report['metadata_only']).lower()}`",
        f"- Payload policy confirmed: `{str(report['payload_policy_confirmed']).lower()}`",
        "",
        "## Source Coverage",
        "",
        f"- Required: {report['source_coverage']['required_count']}",
        f"- Observed: {report['source_coverage']['observed_count']}",
        f"- Missing: {report['source_coverage']['missing_count']}",
        "",
        "## Drift Summary",
        "",
    ]
    drift = report["drift_summary"]
    lines.extend([f"- {key}: {value}" for key, value in drift.items()] or ["- None"])
    lines.extend(["", "## Blockers", ""])
    lines.extend([f"- {item}" for item in report["blockers"]] or ["- None"])
    lines.extend(["", "## Warnings", ""])
    lines.extend([f"- {item}" for item in report["warnings"]] or ["- None"])
    lines.extend(["", "## Blocked Transitions And Non Goals", ""])
    lines.extend([f"- {item}" for item in report["blocked_transitions"]] or ["- None"])
    lines.extend(["", "## Next Actions", ""])
    lines.extend(f"- {item}" for item in report["next_actions"])
    lines.extend(["", "## Downstream Handoff", ""])
    lines.extend(f"- {item}" for item in report["downstream_handoff"])
    lines.append("")
    return "\n".join(lines)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--index", required=True, type=Path)
    parser.add_argument("--diagnostics", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--markdown", required=True, type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    index = _load_json_object(args.index, INDEX_SCHEMA_VERSION)
    diagnostics = _load_json_object(args.diagnostics, DIAGNOSTICS_SCHEMA_VERSION)
    report = build_report(index, diagnostics)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.markdown.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    args.markdown.write_text(render_markdown(report), encoding="utf-8")
    print(f"m198_readiness_report={args.report}")
    print(f"m198_readiness_markdown={args.markdown}")
    print(f"verdict={report['verdict']}")
    return 2 if report["verdict"] == "blocked" else 0


if __name__ == "__main__":
    raise SystemExit(main())
