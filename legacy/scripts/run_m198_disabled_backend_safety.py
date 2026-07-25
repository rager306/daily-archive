#!/usr/bin/env python3
"""Audit disabled graph backend seams for M198 readiness."""

from __future__ import annotations

import argparse
import json
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from research_graph.domain.ports import ProjectionRequest, ProjectionResult
from research_graph.infrastructure.graph.projection_backends import (
    DisabledBackendProjectionAdapter,
    DisabledFalkorProjectionAdapter,
    DisabledLadybugProjectionAdapter,
)
from research_graph.workflows.universal_kb.contracts import CandidatePacket

AUDIT_SCHEMA_VERSION = "m198.disabled_backend_safety.v1"
FORBIDDEN_OUTPUT_TERMS = (
    "raw_text",
    "source_text",
    "chunk_text",
    "paper_text",
    "embedding_payload",
    "vector_payload",
    "secret_value",
)


def _candidate_packet() -> CandidatePacket:
    return CandidatePacket(
        candidate_id="m198-s15-candidate",
        evidence_refs=("artifact:m198-s15-evidence",),
        candidate_type="graph_candidate",
        graph_node_refs=("node:paper:1",),
        graph_edge_refs=("edge:paper:1->claim:1",),
        provenance_refs=("source:m198-s15-fixture",),
    )


def _safety_flags(result: ProjectionResult) -> dict[str, bool]:
    return {key: bool(value) for key, value in result.safety_flags.to_dict().items()}


def _result_summary(name: str, result: ProjectionResult, *, dry_run: bool) -> dict[str, Any]:
    result.assert_no_write()
    return {
        "name": name,
        "backend": result.backend,
        "dry_run": dry_run,
        "node_ref_count": len(result.node_refs),
        "edge_ref_count": len(result.edge_refs),
        "evidence_ref_count": len(result.evidence_refs),
        "provenance_ref_count": len(result.provenance_refs),
        "diagnostic_codes": [diagnostic.code for diagnostic in result.diagnostics],
        "diagnostic_phases": [diagnostic.phase for diagnostic in result.diagnostics],
        "safety_flags": _safety_flags(result),
    }


def adapter_summaries() -> list[dict[str, Any]]:
    request = ProjectionRequest(candidate_packet=_candidate_packet())
    unsafe_backend = "_".join(("api", "key"))
    return [
        _result_summary("disabled_ladybug", DisabledLadybugProjectionAdapter().project(request), dry_run=False),
        _result_summary("disabled_falkor", DisabledFalkorProjectionAdapter().project(request), dry_run=False),
        _result_summary(
            "disabled_ladybug_dry_run",
            DisabledBackendProjectionAdapter(backend="ladybugdb", dry_run=True).project(request),
            dry_run=True,
        ),
        _result_summary(
            "unsafe_backend_name",
            DisabledBackendProjectionAdapter(backend=unsafe_backend).project(request),
            dry_run=False,
        ),
    ]


def _check(name: str, passed: bool, detail: str) -> dict[str, Any]:
    return {"name": name, "passed": passed, "detail": detail}


def build_audit(summaries: list[dict[str, Any]]) -> dict[str, Any]:
    checks: list[dict[str, Any]] = []
    by_name = {str(item.get("name")): item for item in summaries}

    disabled = [item for item in summaries if not item.get("dry_run") and item.get("name") != "unsafe_backend_name"]
    disabled_codes_ok = all("backend_projection_disabled" in (item.get("diagnostic_codes") or []) for item in disabled)
    checks.append(_check("disabled_diagnostics", disabled_codes_ok, "disabled adapters report backend_projection_disabled" if disabled_codes_ok else "disabled diagnostic missing"))

    disabled_refs_ok = all(item.get("node_ref_count") == 0 and item.get("edge_ref_count") == 0 for item in disabled)
    checks.append(_check("disabled_no_refs", disabled_refs_ok, "disabled adapters emit no node or edge refs" if disabled_refs_ok else "disabled adapter emitted refs"))

    dry_run = by_name.get("disabled_ladybug_dry_run", {})
    dry_run_metadata_ok = dry_run.get("node_ref_count") == 1 and dry_run.get("edge_ref_count") == 1 and dry_run.get("evidence_ref_count") == 1
    checks.append(_check("dry_run_metadata_only_refs", dry_run_metadata_ok, "dry-run echoes metadata refs" if dry_run_metadata_ok else "dry-run metadata refs missing"))

    unsafe = by_name.get("unsafe_backend_name", {})
    unsafe_closed = unsafe.get("backend") == "disabled_backend" and "backend_projection_configuration_invalid" in (unsafe.get("diagnostic_codes") or [])
    checks.append(_check("unsafe_backend_fail_closed", unsafe_closed, "unsafe backend fails closed" if unsafe_closed else "unsafe backend did not fail closed"))

    flag_failures: list[str] = []
    for item in summaries:
        flags = item.get("safety_flags") or {}
        for flag_name, value in flags.items():
            if value is not False:
                flag_failures.append(f"{item.get('name')}:{flag_name}={value!r}")
    checks.append(_check("safety_flags_false", not flag_failures, "all safety flags false" if not flag_failures else ", ".join(flag_failures)))

    serialized = json.dumps(summaries, sort_keys=True).lower()
    forbidden_hits = [term for term in FORBIDDEN_OUTPUT_TERMS if term in serialized]
    checks.append(_check("payload_terms_absent", not forbidden_hits, "forbidden payload terms absent" if not forbidden_hits else ", ".join(forbidden_hits)))

    failed_checks = [check for check in checks if not check["passed"]]
    return {
        "schema_version": AUDIT_SCHEMA_VERSION,
        "generated_at": datetime.now(UTC).isoformat(),
        "status": "fail" if failed_checks else "pass",
        "ready": not failed_checks,
        "metadata_only": not forbidden_hits,
        "payload_policy_confirmed": not forbidden_hits,
        "adapter_summaries": summaries,
        "checks": checks,
        "failed_checks": failed_checks,
        "blockers": [str(check["detail"]) for check in failed_checks],
        "warnings": [],
        "downstream_handoff": [
            "S16 end-to-end validation package",
            "S17 operator readiness runbook",
        ],
    }


def render_markdown(audit: dict[str, Any]) -> str:
    lines = [
        "# M198 Disabled Backend Safety",
        "",
        f"- Status: `{audit['status']}`",
        f"- Metadata only: `{str(audit['metadata_only']).lower()}`",
        f"- Payload policy confirmed: `{str(audit['payload_policy_confirmed']).lower()}`",
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
    parser.add_argument("--audit", required=True, type=Path)
    parser.add_argument("--markdown", required=True, type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    audit = build_audit(adapter_summaries())
    args.audit.parent.mkdir(parents=True, exist_ok=True)
    args.markdown.parent.mkdir(parents=True, exist_ok=True)
    args.audit.write_text(json.dumps(audit, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    args.markdown.write_text(render_markdown(audit), encoding="utf-8")
    print(f"m198_disabled_backend_safety={args.audit}")
    print(f"m198_disabled_backend_safety_markdown={args.markdown}")
    print(f"status={audit['status']}")
    return 2 if audit["status"] == "fail" else 0


if __name__ == "__main__":
    raise SystemExit(main())
