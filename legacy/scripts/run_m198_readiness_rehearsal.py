#!/usr/bin/env python3
"""Run a temp-dir M198 readiness rehearsal over index, diagnostics, and report commands."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SUMMARY_SCHEMA_VERSION = "m198.readiness_rehearsal.v1"
EVIDENCE_SCHEMA_VERSION = "m198.readiness_evidence.v1"
REQUIRED_SOURCE_KINDS = (
    "reactive_dry_run",
    "sync_no_write_rehearsal",
    "smoke_boundary",
    "graph_readiness_validate_only",
    "governance_ratchet",
)
REQUIRED_NON_GOALS = (
    "production_graph_import",
    "schema_migration",
    "queue_dependency_semantic_change",
    "smoke_semantic_change",
    "rehearsal_semantic_change",
    "retired_graph_readiness_shim",
    "import_eligible_true",
)


def _evidence(source_kind: str, *, mode: str) -> dict[str, Any]:
    blocked_source = mode == "blocker" and source_kind == "reactive_dry_run"
    diagnostics: dict[str, Any] = {"warnings": [], "blockers": []}
    if blocked_source:
        diagnostics["blockers"] = ["fixture graph write flag violation"]
    if mode == "payload_leak" and source_kind == "smoke_boundary":
        diagnostics["warnings"] = ["vector_payload leak marker for forbidden-term rehearsal"]
    return {
        "schema_version": EVIDENCE_SCHEMA_VERSION,
        "evidence_id": f"m198-s13-{source_kind}",
        "source_kind": source_kind,
        "correlation_id": "m198-s13-rehearsal",
        "status": "blocker" if blocked_source else "pass",
        "drift_class": "blocker" if blocked_source else "expected",
        "timestamp": datetime.now(UTC).isoformat(),
        "graph_writes_allowed": False if not blocked_source else True,
        "schema_migration_allowed": False,
        "import_eligible": False,
        "evidence_refs": [f"artifact://m198/s13/{source_kind}"],
        "diagnostics": diagnostics,
        "non_goals": list(REQUIRED_NON_GOALS),
        "source_checksums": {"fixture": f"sha256:{source_kind}"},
    }


def _write_fixture_evidence(workdir: Path, *, mode: str) -> list[Path]:
    evidence_dir = workdir / "evidence"
    evidence_dir.mkdir(parents=True, exist_ok=True)
    source_kinds = list(REQUIRED_SOURCE_KINDS)
    if mode == "missing_source":
        source_kinds.remove("smoke_boundary")
    paths: list[Path] = []
    for source_kind in source_kinds:
        path = evidence_dir / f"{source_kind}.json"
        path.write_text(json.dumps(_evidence(source_kind, mode=mode), indent=2, sort_keys=True) + "\n", encoding="utf-8")
        paths.append(path)
    return paths


def _run_command(name: str, args: list[str]) -> dict[str, Any]:
    completed = subprocess.run(args, cwd=ROOT, text=True, capture_output=True)
    return {
        "name": name,
        "args": args,
        "exit_code": completed.returncode,
        "stdout": completed.stdout.strip().splitlines(),
        "stderr": completed.stderr.strip().splitlines(),
    }


def _load_json_if_exists(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}
    loaded = json.loads(path.read_text(encoding="utf-8"))
    return loaded if isinstance(loaded, dict) else {}


def run_rehearsal(workdir: Path, *, mode: str) -> dict[str, Any]:
    workdir.mkdir(parents=True, exist_ok=True)
    evidence_paths = _write_fixture_evidence(workdir, mode=mode)
    index_path = workdir / "m198-readiness-index.json"
    diagnostics_path = workdir / "m198-operator-diagnostics.json"
    diagnostics_md = workdir / "m198-operator-diagnostics.md"
    report_path = workdir / "m198-readiness-report.json"
    report_md = workdir / "m198-readiness-report.md"

    index_args = [sys.executable, str(ROOT / "scripts/run_m198_evidence_index.py")]
    for path in evidence_paths:
        index_args.extend(["--evidence", str(path)])
    index_args.extend(["--index", str(index_path)])
    command_log = [_run_command("evidence_index", index_args)]

    if index_path.exists():
        command_log.append(
            _run_command(
                "operator_diagnostics",
                [
                    sys.executable,
                    str(ROOT / "scripts/run_m198_operator_diagnostics.py"),
                    "--index",
                    str(index_path),
                    "--diagnostics",
                    str(diagnostics_path),
                    "--markdown",
                    str(diagnostics_md),
                ],
            )
        )
    if diagnostics_path.exists():
        command_log.append(
            _run_command(
                "readiness_report",
                [
                    sys.executable,
                    str(ROOT / "scripts/run_m198_readiness_report.py"),
                    "--index",
                    str(index_path),
                    "--diagnostics",
                    str(diagnostics_path),
                    "--report",
                    str(report_path),
                    "--markdown",
                    str(report_md),
                ],
            )
        )

    index = _load_json_if_exists(index_path)
    diagnostics = _load_json_if_exists(diagnostics_path)
    report = _load_json_if_exists(report_path)
    final_verdict = str(report.get("verdict") or diagnostics.get("verdict") or index.get("status") or "blocked")
    boundary_confirmations = {
        "graph_writes_allowed": False,
        "schema_migration_allowed": False,
        "import_eligible": False,
        "production_graph_import": False,
        "queue_dependency_semantic_change": False,
        "smoke_semantic_change": False,
        "rehearsal_semantic_change": False,
        "retired_graph_readiness_shim_restored": False,
    }
    return {
        "schema_version": SUMMARY_SCHEMA_VERSION,
        "generated_at": datetime.now(UTC).isoformat(),
        "mode": mode,
        "verdict": final_verdict,
        "ready": final_verdict == "ready",
        "metadata_only": bool(index.get("metadata_only") and diagnostics.get("metadata_only") and report.get("metadata_only")),
        "payload_policy_confirmed": bool(report.get("payload_policy_confirmed", False)),
        "boundary_confirmations": boundary_confirmations,
        "command_log": command_log,
        "artifacts": {
            "evidence_files": [str(path) for path in evidence_paths],
            "index": str(index_path),
            "diagnostics": str(diagnostics_path),
            "diagnostics_markdown": str(diagnostics_md),
            "report": str(report_path),
            "report_markdown": str(report_md),
        },
        "blockers": list(report.get("blockers") or diagnostics.get("blockers") or index.get("blockers") or []),
        "warnings": list(report.get("warnings") or diagnostics.get("warnings") or index.get("warnings") or []),
        "downstream_handoff": [
            "S14 smoke parity audit",
            "S15 disabled backend safety checks",
            "S16 end-to-end validation package",
        ],
    }


def render_markdown(summary: dict[str, Any]) -> str:
    lines = [
        "# M198 Readiness Rehearsal",
        "",
        f"- Mode: `{summary['mode']}`",
        f"- Verdict: `{summary['verdict']}`",
        f"- Ready: `{str(summary['ready']).lower()}`",
        f"- Metadata only: `{str(summary['metadata_only']).lower()}`",
        f"- Payload policy confirmed: `{str(summary['payload_policy_confirmed']).lower()}`",
        "",
        "## Commands",
        "",
    ]
    for command in summary["command_log"]:
        lines.append(f"- {command['name']}: exit {command['exit_code']}")
    lines.extend(["", "## Blockers", ""])
    lines.extend([f"- {item}" for item in summary["blockers"]] or ["- None"])
    lines.extend(["", "## Warnings", ""])
    lines.extend([f"- {item}" for item in summary["warnings"]] or ["- None"])
    lines.extend(["", "## Downstream Handoff", ""])
    lines.extend(f"- {item}" for item in summary["downstream_handoff"])
    lines.append("")
    return "\n".join(lines)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workdir", required=True, type=Path)
    parser.add_argument("--summary", required=True, type=Path)
    parser.add_argument("--markdown", required=True, type=Path)
    parser.add_argument("--mode", choices=("ready", "blocker", "missing_source", "payload_leak"), default="ready")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    summary = run_rehearsal(args.workdir, mode=args.mode)
    args.summary.parent.mkdir(parents=True, exist_ok=True)
    args.markdown.parent.mkdir(parents=True, exist_ok=True)
    args.summary.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    args.markdown.write_text(render_markdown(summary), encoding="utf-8")
    print(f"m198_readiness_rehearsal={args.summary}")
    print(f"m198_readiness_rehearsal_markdown={args.markdown}")
    print(f"verdict={summary['verdict']}")
    return 2 if summary["verdict"] == "blocked" else 0


if __name__ == "__main__":
    raise SystemExit(main())
