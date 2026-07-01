#!/usr/bin/env python3
"""Audit smoke-boundary parity from an M198 readiness rehearsal summary."""

from __future__ import annotations

import argparse
import json
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

AUDIT_SCHEMA_VERSION = "m198.smoke_parity_audit.v1"
REHEARSAL_SCHEMA_VERSION = "m198.readiness_rehearsal.v1"
INDEX_SCHEMA_VERSION = "m198.readiness_evidence_index.v1"
REQUIRED_COMMANDS = ("evidence_index", "operator_diagnostics", "readiness_report")
REQUIRED_FALSE_BOUNDARIES = (
    "graph_writes_allowed",
    "schema_migration_allowed",
    "import_eligible",
    "production_graph_import",
    "queue_dependency_semantic_change",
    "smoke_semantic_change",
    "rehearsal_semantic_change",
    "retired_graph_readiness_shim_restored",
)


def _load_json_object(path: Path, expected_schema: str) -> dict[str, Any]:
    loaded = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(loaded, dict):
        raise ValueError(f"{path} must contain a JSON object")
    if loaded.get("schema_version") != expected_schema:
        raise ValueError(f"expected {expected_schema}, got {loaded.get('schema_version')!r}")
    return loaded


def _load_index(rehearsal: dict[str, Any]) -> dict[str, Any]:
    index_path = Path(str((rehearsal.get("artifacts") or {}).get("index", "")))
    if not index_path.exists():
        return {"schema_version": INDEX_SCHEMA_VERSION, "missing_index": str(index_path), "entries": []}
    return _load_json_object(index_path, INDEX_SCHEMA_VERSION)


def _check(name: str, passed: bool, detail: str) -> dict[str, Any]:
    return {"name": name, "passed": passed, "detail": detail}


def build_audit(rehearsal: dict[str, Any], index: dict[str, Any]) -> dict[str, Any]:
    checks: list[dict[str, Any]] = []
    command_names = [str(item.get("name")) for item in rehearsal.get("command_log") or [] if isinstance(item, dict)]
    missing_commands = [name for name in REQUIRED_COMMANDS if name not in command_names]
    checks.append(_check("command_chain", not missing_commands, "missing commands: " + ", ".join(missing_commands) if missing_commands else "all commands present"))

    entries = [entry for entry in index.get("entries") or [] if isinstance(entry, dict)]
    source_kinds = {str(entry.get("source_kind")) for entry in entries}
    checks.append(_check("smoke_boundary_source", "smoke_boundary" in source_kinds, "smoke_boundary present" if "smoke_boundary" in source_kinds else "smoke_boundary missing"))

    non_goals = {str(item) for item in index.get("non_goal_coverage") or []}
    checks.append(_check("smoke_semantic_change_blocked", "smoke_semantic_change" in non_goals, "smoke semantic change remains non-goal" if "smoke_semantic_change" in non_goals else "smoke semantic change non-goal missing"))

    boundary_confirmations = rehearsal.get("boundary_confirmations") or {}
    boundary_failures = [key for key in REQUIRED_FALSE_BOUNDARIES if boundary_confirmations.get(key) is not False]
    checks.append(_check("no_write_import_boundaries", not boundary_failures, "boundary failures: " + ", ".join(boundary_failures) if boundary_failures else "all boundaries false"))

    metadata_only = rehearsal.get("metadata_only") is True and index.get("metadata_only") is True
    checks.append(_check("metadata_only", metadata_only, "metadata-only confirmed" if metadata_only else "metadata-only not confirmed"))

    payload_ok = rehearsal.get("payload_policy_confirmed") is True and (index.get("payload_policy") or {}).get("stores_payload_text") is False
    checks.append(_check("payload_policy", payload_ok, "payload policy confirmed" if payload_ok else "payload policy not confirmed"))

    blockers = [str(item) for item in (rehearsal.get("blockers") or [])]
    readiness_verdict = str(rehearsal.get("verdict"))
    verdict_consistent = (readiness_verdict == "ready" and not blockers) or (readiness_verdict == "blocked" and bool(blockers))
    checks.append(_check("verdict_propagation", verdict_consistent, "verdict matches blockers" if verdict_consistent else "verdict does not match blockers"))

    failed_checks = [check for check in checks if not check["passed"]]
    status = "fail" if failed_checks or readiness_verdict == "blocked" else "pass"
    return {
        "schema_version": AUDIT_SCHEMA_VERSION,
        "generated_at": datetime.now(UTC).isoformat(),
        "status": status,
        "readiness_verdict": readiness_verdict,
        "ready": status == "pass",
        "checks": checks,
        "failed_checks": failed_checks,
        "blockers": blockers + [str(check["detail"]) for check in failed_checks],
        "warnings": [str(item) for item in (rehearsal.get("warnings") or [])],
        "smoke_boundary_present": "smoke_boundary" in source_kinds,
        "metadata_only": metadata_only,
        "payload_policy_confirmed": payload_ok,
        "downstream_handoff": [
            "S15 disabled backend safety checks",
            "S16 end-to-end validation package",
        ],
    }


def render_markdown(audit: dict[str, Any]) -> str:
    lines = [
        "# M198 Smoke Parity Audit",
        "",
        f"- Status: `{audit['status']}`",
        f"- Readiness verdict: `{audit['readiness_verdict']}`",
        f"- Smoke boundary present: `{str(audit['smoke_boundary_present']).lower()}`",
        f"- Metadata only: `{str(audit['metadata_only']).lower()}`",
        "",
        "## Checks",
        "",
    ]
    for check in audit["checks"]:
        marker = "PASS" if check["passed"] else "FAIL"
        lines.append(f"- {marker}: {check['name']} - {check['detail']}")
    lines.extend(["", "## Blockers", ""])
    lines.extend([f"- {item}" for item in audit["blockers"]] or ["- None"])
    lines.extend(["", "## Downstream Handoff", ""])
    lines.extend(f"- {item}" for item in audit["downstream_handoff"])
    lines.append("")
    return "\n".join(lines)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rehearsal", required=True, type=Path)
    parser.add_argument("--audit", required=True, type=Path)
    parser.add_argument("--markdown", required=True, type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    rehearsal = _load_json_object(args.rehearsal, REHEARSAL_SCHEMA_VERSION)
    audit = build_audit(rehearsal, _load_index(rehearsal))
    args.audit.parent.mkdir(parents=True, exist_ok=True)
    args.markdown.parent.mkdir(parents=True, exist_ok=True)
    args.audit.write_text(json.dumps(audit, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    args.markdown.write_text(render_markdown(audit), encoding="utf-8")
    print(f"m198_smoke_parity_audit={args.audit}")
    print(f"m198_smoke_parity_markdown={args.markdown}")
    print(f"status={audit['status']}")
    return 2 if audit["status"] == "fail" else 0


if __name__ == "__main__":
    raise SystemExit(main())
