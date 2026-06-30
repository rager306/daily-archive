#!/usr/bin/env python3
"""Convert Universal KB no-write rehearsal artifacts into M198 readiness evidence."""

from __future__ import annotations

import argparse
import json
from datetime import UTC, datetime
from hashlib import sha256
from pathlib import Path
from typing import Any

from research_graph.workflows.universal_kb.rehearsal import run_universal_kb_no_write_rehearsal

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m198-readiness-evidence-contract.json"


def _load_contract() -> dict[str, Any]:
    return json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))


def _load_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise FileNotFoundError(f"required rehearsal artifact missing: {path.name}")
    loaded = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(loaded, dict):
        raise ValueError(f"expected object artifact: {path.name}")
    return loaded


def _sha256(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _json_artifacts(artifact_dir: Path) -> list[Path]:
    return sorted(path for path in artifact_dir.glob("*.json") if path.is_file())


def _reject_payload_terms(artifact_dir: Path, forbidden_terms: list[str]) -> None:
    text = "\n".join(path.read_text(encoding="utf-8") for path in _json_artifacts(artifact_dir)).lower()
    for term in forbidden_terms:
        if term.lower() in text:
            raise ValueError(f"forbidden payload term found: {term}")


def _require_false(name: str, value: Any) -> None:
    if value is not False:
        raise ValueError(f"{name} must be false for sync no-write rehearsal evidence")


def build_evidence(artifact_dir: Path, *, correlation_id: str) -> dict[str, Any]:
    contract = _load_contract()
    summary = _load_json(artifact_dir / "summary.json")
    handoff = _load_json(artifact_dir / "readiness_handoff.json")
    schema_gate = _load_json(artifact_dir / "schema_gate_result.json")
    projection = _load_json(artifact_dir / "projection_result.json")
    queue_inspect = _load_json(artifact_dir / "queue_inspect.json")
    _reject_payload_terms(artifact_dir, contract["forbidden_payload_terms"])

    _require_false("graph_write_allowed", summary.get("graph_write_allowed"))
    _require_false("production_import_attempted", summary.get("production_import_attempted"))
    _require_false("promotion_allowed", summary.get("promotion_allowed"))
    _require_false("schema_gate_migration_required", summary.get("schema_gate_migration_required"))
    _require_false("projection_import_eligible", summary.get("projection_import_eligible"))
    _require_false("handoff.graph_write_allowed", handoff.get("graph_write_allowed"))
    _require_false("handoff.production_import_attempted", handoff.get("production_import_attempted"))
    _require_false("handoff.promotion_allowed", handoff.get("promotion_allowed"))
    _require_false("schema_gate.migration_required", schema_gate.get("migration_required"))

    artifact_paths = [Path(path) for path in summary.get("artifact_paths", [])]
    queue_sqlite = artifact_dir / "queue.sqlite"
    queue_events = artifact_dir / "queue_events.json"
    refs = sorted(str(path) for path in artifact_paths if path.name != "queue.sqlite")
    refs.extend([str(queue_sqlite), str(artifact_dir / "queue_inspect.json")])
    evidence_refs = sorted(set(refs))
    checksums = {
        str(path): _sha256(path)
        for path in [*_json_artifacts(artifact_dir), queue_sqlite]
        if path.exists()
    }
    events = queue_inspect.get("events") or []
    job = queue_inspect.get("job") or {}

    return {
        "schema_version": contract["schema_version"],
        "evidence_id": "m198-sync-no-write-rehearsal-probe",
        "source_kind": "sync_no_write_rehearsal",
        "correlation_id": correlation_id,
        "status": "pass",
        "drift_class": "not_applicable",
        "timestamp": datetime.now(UTC).isoformat(),
        "graph_writes_allowed": False,
        "schema_migration_allowed": False,
        "import_eligible": False,
        "evidence_refs": evidence_refs,
        "diagnostics": {
            "artifact_count": summary.get("artifact_count"),
            "candidate_id": summary.get("candidate_id"),
            "queue_job_id": summary.get("queue_job_id"),
            "queue_status": summary.get("queue_status"),
            "queue_event_count": len(events) if isinstance(events, list) else 0,
            "queue_job_present": bool(job),
            "queue_sqlite_present": queue_sqlite.exists(),
            "standalone_queue_events_present": queue_events.exists(),
            "schema_gate_accepted": schema_gate.get("accepted"),
            "schema_gate_migration_required": schema_gate.get("migration_required"),
            "projection_backend": projection.get("backend"),
            "promotion_allowed": summary.get("promotion_allowed"),
            "production_import_attempted": summary.get("production_import_attempted"),
            "projection_import_eligible": summary.get("projection_import_eligible"),
        },
        "non_goals": contract["blocked_transitions"],
        "source_command": "uv run python scripts/run_m198_sync_rehearsal_probe.py --artifact-dir <dir> --evidence <evidence.json>",
        "source_artifact_refs": evidence_refs,
        "source_checksums": checksums,
        "queue_artifact_present": queue_sqlite.exists(),
        "standalone_queue_events_present": queue_events.exists(),
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact-dir", required=True, type=Path)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--correlation-id", default="m198-sync-rehearsal-probe")
    parser.add_argument("--skip-run", action="store_true", help="Build evidence from an existing artifact directory.")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    args.artifact_dir.mkdir(parents=True, exist_ok=True)
    if not args.skip_run:
        run_universal_kb_no_write_rehearsal(args.artifact_dir)
    evidence = build_evidence(args.artifact_dir, correlation_id=args.correlation_id)
    args.evidence.parent.mkdir(parents=True, exist_ok=True)
    args.evidence.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"m198_readiness_evidence={args.evidence}")
    print(f"queue_artifact_present={evidence['queue_artifact_present']}")
    print(f"standalone_queue_events_present={evidence['standalone_queue_events_present']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
