#!/usr/bin/env python3
"""Build the M198 end-to-end readiness validation package."""

from __future__ import annotations

import argparse
import json
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

PACKAGE_SCHEMA_VERSION = "m198.validation_package.v1"
EXPECTED_SCHEMAS = {
    "impact_gates": "m198.gitnexus_impact_gates.v1",
    "rehearsal": "m198.readiness_rehearsal.v1",
    "smoke_parity": "m198.smoke_parity_audit.v1",
    "disabled_backend": "m198.disabled_backend_safety.v1",
}
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


def _load(path: Path, expected_schema: str) -> tuple[dict[str, Any], list[str]]:
    if not path.exists():
        return {}, [f"missing artifact: {path}"]
    loaded = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(loaded, dict):
        return {}, [f"artifact is not a JSON object: {path}"]
    schema = loaded.get("schema_version")
    if schema != expected_schema:
        return loaded, [f"{path} expected {expected_schema}, got {schema!r}"]
    return loaded, []


def _input_summary(name: str, path: Path, data: dict[str, Any], errors: list[str]) -> dict[str, Any]:
    status = str(data.get("status") or data.get("verdict") or "unknown") if data else "missing"
    ready = data.get("ready")
    if ready is None:
        ready = status in {"pass", "ready"}
    return {
        "name": name,
        "path": str(path),
        "schema_version": data.get("schema_version"),
        "status": status,
        "ready": bool(ready),
        "errors": errors,
    }


def _boundary_errors(rehearsal: dict[str, Any]) -> list[str]:
    boundaries = rehearsal.get("boundary_confirmations") or {}
    return [f"boundary {key} is not false" for key in REQUIRED_FALSE_BOUNDARIES if boundaries.get(key) is not False]


def _gate_summary(impact_gates: dict[str, Any]) -> dict[str, Any]:
    gates = impact_gates.get("gates") or []
    gate_ids = [str(gate.get("id")) for gate in gates if isinstance(gate, dict)]
    return {
        "repo": impact_gates.get("repo"),
        "gate_count": len(gate_ids),
        "gate_ids": gate_ids,
        "detect_changes_repo_required": bool((impact_gates.get("detect_changes") or {}).get("repo_required")),
        "refresh_command": (impact_gates.get("index_refresh") or {}).get("command"),
    }


def build_package(inputs: dict[str, tuple[Path, dict[str, Any], list[str]]]) -> dict[str, Any]:
    summaries: list[dict[str, Any]] = []
    blockers: list[str] = []
    warnings: list[str] = []
    for name, (path, data, errors) in inputs.items():
        summaries.append(_input_summary(name, path, data, errors))
        blockers.extend(errors)
        blockers.extend(str(item) for item in (data.get("blockers") or []))
        warnings.extend(str(item) for item in (data.get("warnings") or []))
        status = str(data.get("status") or data.get("verdict") or "")
        if status in {"fail", "blocked"}:
            blockers.append(f"{name} status is {status}")
        if data.get("metadata_only") is False:
            blockers.append(f"{name} metadata_only is false")
        if data.get("payload_policy_confirmed") is False:
            blockers.append(f"{name} payload_policy_confirmed is false")

    rehearsal = inputs.get("rehearsal", (Path(""), {}, []))[1]
    boundary_errors = _boundary_errors(rehearsal)
    blockers.extend(boundary_errors)

    impact_gates = inputs.get("impact_gates", (Path(""), {}, []))[1]
    gate_summary = _gate_summary(impact_gates)
    if gate_summary["refresh_command"] != "gitnexus analyze":
        blockers.append("GitNexus refresh command is not gitnexus analyze")
    if gate_summary["detect_changes_repo_required"] is not True:
        blockers.append("GitNexus detect_changes repo scoping is not required")

    unique_blockers = sorted(set(blockers))
    unique_warnings = sorted(set(warnings))
    status = "fail" if unique_blockers else "pass"
    return {
        "schema_version": PACKAGE_SCHEMA_VERSION,
        "generated_at": datetime.now(UTC).isoformat(),
        "status": status,
        "ready": status == "pass",
        "input_summaries": summaries,
        "gitnexus_gate_summary": gate_summary,
        "boundary_confirmations": rehearsal.get("boundary_confirmations") or {},
        "blockers": unique_blockers,
        "warnings": unique_warnings,
        "metadata_only": all((data.get("metadata_only", True) is not False) for _, data, _ in inputs.values()),
        "payload_policy_confirmed": all(
            (data.get("payload_policy_confirmed", True) is not False) for _, data, _ in inputs.values()
        ),
        "downstream_handoff": [
            "S17 operator readiness runbook",
            "S18 milestone closeout readiness",
        ],
    }


def render_markdown(package: dict[str, Any]) -> str:
    lines = [
        "# M198 Validation Package",
        "",
        f"- Status: `{package['status']}`",
        f"- Ready: `{str(package['ready']).lower()}`",
        f"- Metadata only: `{str(package['metadata_only']).lower()}`",
        f"- Payload policy confirmed: `{str(package['payload_policy_confirmed']).lower()}`",
        "",
        "## Inputs",
        "",
    ]
    for summary in package["input_summaries"]:
        lines.append(f"- {summary['name']}: `{summary['status']}` at `{summary['path']}`")
    lines.extend(["", "## GitNexus Gates", ""])
    lines.append(f"- Repo: `{package['gitnexus_gate_summary']['repo']}`")
    lines.append(f"- Gate count: {package['gitnexus_gate_summary']['gate_count']}")
    lines.extend(["", "## Blockers", ""])
    lines.extend([f"- {item}" for item in package["blockers"]] or ["- None"])
    lines.extend(["", "## Warnings", ""])
    lines.extend([f"- {item}" for item in package["warnings"]] or ["- None"])
    lines.extend(["", "## Downstream Handoff", ""])
    lines.extend(f"- {item}" for item in package["downstream_handoff"])
    lines.append("")
    return "\n".join(lines)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--impact-gates", required=True, type=Path)
    parser.add_argument("--rehearsal", required=True, type=Path)
    parser.add_argument("--smoke-parity", required=True, type=Path)
    parser.add_argument("--disabled-backend", required=True, type=Path)
    parser.add_argument("--package", required=True, type=Path)
    parser.add_argument("--markdown", required=True, type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    paths = {
        "impact_gates": args.impact_gates,
        "rehearsal": args.rehearsal,
        "smoke_parity": args.smoke_parity,
        "disabled_backend": args.disabled_backend,
    }
    inputs = {name: (path, *_load(path, EXPECTED_SCHEMAS[name])) for name, path in paths.items()}
    package = build_package(inputs)
    args.package.parent.mkdir(parents=True, exist_ok=True)
    args.markdown.parent.mkdir(parents=True, exist_ok=True)
    args.package.write_text(json.dumps(package, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    args.markdown.write_text(render_markdown(package), encoding="utf-8")
    print(f"m198_validation_package={args.package}")
    print(f"m198_validation_package_markdown={args.markdown}")
    print(f"status={package['status']}")
    return 2 if package["status"] == "fail" else 0


if __name__ == "__main__":
    raise SystemExit(main())
