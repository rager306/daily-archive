#!/usr/bin/env python3
"""Render operator diagnostics from the M198 metadata-only evidence index."""

from __future__ import annotations

import argparse
import json
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

DIAGNOSTICS_SCHEMA_VERSION = "m198.operator_diagnostics.v1"
INDEX_SCHEMA_VERSION = "m198.readiness_evidence_index.v1"


def _load_index(path: Path) -> dict[str, Any]:
    loaded = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(loaded, dict):
        raise ValueError("operator diagnostics input must be a JSON object")
    if loaded.get("schema_version") != INDEX_SCHEMA_VERSION:
        raise ValueError(f"expected {INDEX_SCHEMA_VERSION}, got {loaded.get('schema_version')!r}")
    return loaded


def _payload_policy_blockers(index: dict[str, Any]) -> list[str]:
    blockers: list[str] = []
    if index.get("metadata_only") is not True:
        blockers.append("index metadata_only must be true")
    policy = index.get("payload_policy") or {}
    required_false = (
        "stores_payload_text",
        "stores_embeddings",
        "stores_vectors",
        "stores_credentials",
        "stores_queue_database_bytes",
    )
    for key in required_false:
        if policy.get(key) is not False:
            blockers.append(f"payload_policy.{key} must be false")
    return blockers


def _coverage(index: dict[str, Any]) -> dict[str, Any]:
    required = list(index.get("required_source_kinds") or [])
    observed = list(index.get("observed_source_kinds") or [])
    missing = list(index.get("missing_source_kinds") or [])
    return {
        "required_count": len(required),
        "observed_count": len(observed),
        "missing_count": len(missing),
        "required_source_kinds": required,
        "observed_source_kinds": observed,
        "missing_source_kinds": missing,
    }


def build_diagnostics(index: dict[str, Any]) -> dict[str, Any]:
    blockers = [str(item) for item in (index.get("blockers") or [])]
    blockers.extend(_payload_policy_blockers(index))
    warnings = [str(item) for item in (index.get("warnings") or [])]
    coverage = _coverage(index)
    if coverage["missing_source_kinds"]:
        blockers.append("missing source kinds: " + ", ".join(coverage["missing_source_kinds"]))

    if blockers or index.get("status") == "fail":
        verdict = "blocked"
    elif warnings:
        verdict = "needs_attention"
    else:
        verdict = "ready"

    next_actions = _next_actions(verdict, blockers, warnings, coverage)
    return {
        "schema_version": DIAGNOSTICS_SCHEMA_VERSION,
        "generated_at": datetime.now(UTC).isoformat(),
        "verdict": verdict,
        "ready": verdict == "ready",
        "index_status": index.get("status"),
        "source_coverage": coverage,
        "entry_count": index.get("entry_count"),
        "warnings": warnings,
        "blockers": blockers,
        "next_actions": next_actions,
        "blocked_transitions": sorted(index.get("non_goal_coverage") or []),
        "payload_policy_confirmed": not _payload_policy_blockers(index),
        "metadata_only": index.get("metadata_only") is True,
    }


def _next_actions(verdict: str, blockers: list[str], warnings: list[str], coverage: dict[str, Any]) -> list[str]:
    if verdict == "ready":
        return [
            "Proceed to S10 readiness report synthesis using the metadata-only diagnostics and index.",
            "Do not enable production graph import or schema migration in this milestone.",
        ]
    if verdict == "needs_attention":
        return [
            "Review indexed warnings before S10 report synthesis.",
            "Preserve no-write/import-blocked boundaries while resolving warning context.",
        ]
    actions = ["Stop readiness promotion until blockers are resolved."]
    if coverage.get("missing_source_kinds"):
        actions.append("Regenerate missing producer evidence: " + ", ".join(coverage["missing_source_kinds"]))
    if any("payload_policy" in blocker or "metadata_only" in blocker for blocker in blockers):
        actions.append("Fix evidence index payload policy before any report synthesis.")
    if any("import" in blocker for blocker in blockers):
        actions.append("Keep import eligibility blocked and inspect the flagged source evidence.")
    if warnings:
        actions.append("Review warnings after blockers are cleared.")
    return actions


def render_markdown(diagnostics: dict[str, Any]) -> str:
    lines = [
        "# M198 Operator Diagnostics",
        "",
        f"- Verdict: `{diagnostics['verdict']}`",
        f"- Ready: `{str(diagnostics['ready']).lower()}`",
        f"- Index status: `{diagnostics['index_status']}`",
        f"- Metadata only: `{str(diagnostics['metadata_only']).lower()}`",
        f"- Payload policy confirmed: `{str(diagnostics['payload_policy_confirmed']).lower()}`",
        "",
        "## Source Coverage",
        "",
        f"- Required: {diagnostics['source_coverage']['required_count']}",
        f"- Observed: {diagnostics['source_coverage']['observed_count']}",
        f"- Missing: {diagnostics['source_coverage']['missing_count']}",
        "",
        "## Blockers",
        "",
    ]
    blockers = diagnostics["blockers"]
    lines.extend([f"- {item}" for item in blockers] or ["- None"])
    lines.extend(["", "## Warnings", ""])
    warnings = diagnostics["warnings"]
    lines.extend([f"- {item}" for item in warnings] or ["- None"])
    lines.extend(["", "## Next Actions", ""])
    lines.extend(f"- {item}" for item in diagnostics["next_actions"])
    lines.extend(["", "## Blocked Transitions", ""])
    lines.extend(f"- {item}" for item in diagnostics["blocked_transitions"])
    lines.append("")
    return "\n".join(lines)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--index", required=True, type=Path)
    parser.add_argument("--diagnostics", required=True, type=Path)
    parser.add_argument("--markdown", required=True, type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    diagnostics = build_diagnostics(_load_index(args.index))
    args.diagnostics.parent.mkdir(parents=True, exist_ok=True)
    args.markdown.parent.mkdir(parents=True, exist_ok=True)
    args.diagnostics.write_text(json.dumps(diagnostics, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    args.markdown.write_text(render_markdown(diagnostics), encoding="utf-8")
    print(f"m198_operator_diagnostics={args.diagnostics}")
    print(f"m198_operator_markdown={args.markdown}")
    print(f"verdict={diagnostics['verdict']}")
    return 2 if diagnostics["verdict"] == "blocked" else 0


if __name__ == "__main__":
    raise SystemExit(main())
