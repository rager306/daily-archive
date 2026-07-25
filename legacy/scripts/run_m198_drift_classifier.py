#!/usr/bin/env python3
"""Classify drift across M198 readiness evidence producers."""

from __future__ import annotations

import argparse
import json
from datetime import UTC, datetime
from hashlib import sha256
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m198-readiness-evidence-contract.json"
REQUIRED_SOURCE_KINDS = (
    "reactive_dry_run",
    "sync_no_write_rehearsal",
    "smoke_boundary",
    "graph_readiness_validate_only",
)


def _load_contract() -> dict[str, Any]:
    return json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))


def _sha256(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _load_evidence(path: Path) -> dict[str, Any]:
    loaded = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(loaded, dict):
        raise ValueError(f"expected evidence object: {path}")
    loaded["_path"] = str(path)
    return loaded


def _has_forbidden_terms(evidence: dict[str, Any], forbidden_terms: list[str]) -> list[str]:
    text = json.dumps(evidence, sort_keys=True).lower()
    return [term for term in forbidden_terms if term.lower() in text]


def _flag_blockers(evidence: dict[str, Any]) -> list[str]:
    blockers: list[str] = []
    for field in ("graph_writes_allowed", "schema_migration_allowed", "import_eligible"):
        if evidence.get(field) is not False:
            blockers.append(f"{evidence.get('source_kind')} has {field}={evidence.get(field)!r}")
    if evidence.get("status") != "pass":
        blockers.append(f"{evidence.get('source_kind')} status is {evidence.get('status')!r}")
    if not evidence.get("evidence_refs"):
        blockers.append(f"{evidence.get('source_kind')} has no evidence_refs")
    return blockers


def _source_specific_blockers(evidence_by_kind: dict[str, dict[str, Any]]) -> list[str]:
    blockers: list[str] = []
    reactive = evidence_by_kind.get("reactive_dry_run", {})
    if reactive:
        diagnostics = reactive.get("diagnostics") or {}
        if not diagnostics.get("event_count"):
            blockers.append("reactive_dry_run has no event_count diagnostic")
        if reactive.get("queue_artifact_present") is not False:
            blockers.append("reactive_dry_run queue_artifact_present must be false")

    sync = evidence_by_kind.get("sync_no_write_rehearsal", {})
    if sync:
        diagnostics = sync.get("diagnostics") or {}
        if sync.get("queue_artifact_present") is not True:
            blockers.append("sync_no_write_rehearsal queue_artifact_present must be true")
        if sync.get("standalone_queue_events_present") is not False:
            blockers.append("sync_no_write_rehearsal standalone_queue_events_present must be false")
        if diagnostics.get("schema_gate_migration_required") is not False:
            blockers.append("sync_no_write_rehearsal schema_gate_migration_required must be false")

    smoke = evidence_by_kind.get("smoke_boundary", {})
    if smoke:
        diagnostics = smoke.get("diagnostics") or {}
        if smoke.get("queue_status") != "ready" and diagnostics.get("queue_status") != "ready":
            blockers.append("smoke_boundary queue_status must be ready")
        if diagnostics.get("metadata_only") is not True:
            blockers.append("smoke_boundary metadata_only must be true")

    graph = evidence_by_kind.get("graph_readiness_validate_only", {})
    if graph:
        diagnostics = graph.get("diagnostics") or {}
        if diagnostics.get("validate_only") is not True:
            blockers.append("graph_readiness_validate_only validate_only must be true")
        if diagnostics.get("require_completed_review") is not True:
            blockers.append("graph_readiness_validate_only require_completed_review must be true")
        if diagnostics.get("validator_ok") is not True:
            blockers.append("graph_readiness_validate_only validator_ok must be true")
        if diagnostics.get("retired_alias_absent") is not True:
            blockers.append("graph_readiness_validate_only retired_alias_absent must be true")
    return blockers


def classify(paths: list[Path], *, correlation_id: str) -> dict[str, Any]:
    contract = _load_contract()
    evidence_items = [_load_evidence(path) for path in paths]
    evidence_by_kind: dict[str, dict[str, Any]] = {}
    warnings: list[str] = []
    blockers: list[str] = []

    for evidence in evidence_items:
        source_kind = str(evidence.get("source_kind"))
        if source_kind in evidence_by_kind:
            blockers.append(f"duplicate source_kind: {source_kind}")
        evidence_by_kind[source_kind] = evidence
        blockers.extend(_flag_blockers(evidence))
        for term in _has_forbidden_terms(evidence, contract["forbidden_payload_terms"]):
            blockers.append(f"{source_kind} contains forbidden payload term: {term}")

    for source_kind in REQUIRED_SOURCE_KINDS:
        if source_kind not in evidence_by_kind:
            blockers.append(f"missing required source kind: {source_kind}")

    extra_sources = sorted(kind for kind in evidence_by_kind if kind not in REQUIRED_SOURCE_KINDS)
    if extra_sources:
        warnings.append(f"extra source kinds ignored: {', '.join(extra_sources)}")

    blockers.extend(_source_specific_blockers(evidence_by_kind))
    drift_class = "blocker" if blockers else "warning" if warnings else "expected"
    status = "fail" if blockers else "pass"
    source_paths = [str(path) for path in paths]

    return {
        "schema_version": contract["schema_version"],
        "evidence_id": "m198-readiness-drift-classifier",
        "source_kind": "governance_ratchet",
        "correlation_id": correlation_id,
        "status": status,
        "drift_class": drift_class,
        "timestamp": datetime.now(UTC).isoformat(),
        "graph_writes_allowed": False,
        "schema_migration_allowed": False,
        "import_eligible": False,
        "evidence_refs": source_paths,
        "diagnostics": {
            "required_source_kinds": list(REQUIRED_SOURCE_KINDS),
            "observed_source_kinds": sorted(evidence_by_kind),
            "expected_drift": [
                "reactive_dry_run has no queue artifact",
                "sync_no_write_rehearsal has queue.sqlite and no standalone queue_events.json",
                "smoke_boundary has queue_status ready",
                "graph_readiness_validate_only records retired alias absence",
            ],
            "warnings": warnings,
            "blockers": blockers,
        },
        "non_goals": contract["blocked_transitions"],
        "source_command": "uv run python scripts/run_m198_drift_classifier.py --evidence <json>... --report <report.json>",
        "source_artifact_refs": source_paths,
        "source_checksums": {str(path): _sha256(path) for path in paths},
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True, type=Path, action="append")
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--correlation-id", default="m198-drift-classifier")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    report = classify(args.evidence, correlation_id=args.correlation_id)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"m198_drift_report={args.report}")
    print(f"drift_class={report['drift_class']}")
    return 2 if report["drift_class"] == "blocker" else 0


if __name__ == "__main__":
    raise SystemExit(main())
