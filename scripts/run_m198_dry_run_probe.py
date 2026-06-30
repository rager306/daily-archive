#!/usr/bin/env python3
"""Convert M197 dry-run JSONL events into M198 readiness evidence."""

from __future__ import annotations

import argparse
import json
from datetime import UTC, datetime
from hashlib import sha256
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m198-readiness-evidence-contract.json"


def _load_contract() -> dict[str, Any]:
    return json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))


def _sha256(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _load_events(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        raise FileNotFoundError(f"events file not found: {path}")
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def _reject_payload_terms(events: list[dict[str, Any]], forbidden_terms: list[str]) -> None:
    text = json.dumps(events).lower()
    for term in forbidden_terms:
        if term.lower() in text:
            raise ValueError(f"forbidden payload term found: {term}")


def _require_false_flags(events: list[dict[str, Any]]) -> None:
    for event in events:
        for field in ("graph_writes_allowed", "schema_migration_allowed", "import_eligible"):
            if event.get(field) is not False:
                raise ValueError(f"{field} must be false for reactive dry-run evidence")


def build_evidence(events_path: Path, *, correlation_id: str) -> dict[str, Any]:
    contract = _load_contract()
    events = _load_events(events_path)
    _reject_payload_terms(events, contract["forbidden_payload_terms"])
    _require_false_flags(events)
    if not events:
        raise ValueError("events file is empty")

    event_types = [str(event["event_type"]) for event in events]
    completed = [event for event in events if event.get("event_type") == "stage.completed"]
    source_artifact_refs = sorted(
        {
            ref
            for event in completed
            for ref in (event.get("child_artifact_refs") or event.get("artifact_refs") or [])
        }
    )
    return {
        "schema_version": contract["schema_version"],
        "evidence_id": "m198-reactive-dry-run-probe",
        "source_kind": "reactive_dry_run",
        "correlation_id": correlation_id,
        "status": "pass",
        "drift_class": "not_applicable",
        "timestamp": datetime.now(UTC).isoformat(),
        "graph_writes_allowed": False,
        "schema_migration_allowed": False,
        "import_eligible": False,
        "evidence_refs": [str(events_path)],
        "diagnostics": {
            "event_count": len(events),
            "event_types": event_types,
            "completed_stage_count": len(completed),
            "queue_artifact_present": False,
            "standalone_queue_events_present": False,
        },
        "non_goals": contract["blocked_transitions"],
        "source_command": "uv run python scripts/run_m197_reactive_dry_run.py --events <events.jsonl>",
        "source_artifact_refs": source_artifact_refs,
        "source_checksums": {str(events_path): _sha256(events_path)},
        "event_count": len(events),
        "queue_artifact_present": False,
        "standalone_queue_events_present": False,
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--events", required=True, type=Path)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--correlation-id", default="m198-dry-run-probe")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    evidence = build_evidence(args.events, correlation_id=args.correlation_id)
    args.evidence.parent.mkdir(parents=True, exist_ok=True)
    args.evidence.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"m198_readiness_evidence={args.evidence}")
    print(f"event_count={evidence['event_count']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
