from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from research_graph.workflows.universal_kb.rehearsal import run_universal_kb_no_write_rehearsal

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m197-reactive-event-contract.json"
SCRIPT = ROOT / "scripts/run_m197_reactive_dry_run.py"


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _load_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def _run_dry_job(tmp_path: Path, *, job_id: str, correlation_id: str) -> list[dict]:
    events_path = tmp_path / job_id / "events.jsonl"
    subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--events",
            str(events_path),
            "--job-id",
            job_id,
            "--correlation-id",
            correlation_id,
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=True,
    )
    return _load_jsonl(events_path)


def test_realistic_no_write_rehearsal_runs_multiple_reactive_jobs_and_sync_baseline(
    tmp_path: Path,
) -> None:
    contract = _load(CONTRACT_PATH)
    all_events = []
    for index in range(3):
        all_events.extend(
            _run_dry_job(
                tmp_path / "reactive",
                job_id=f"m197-realistic-job-{index}",
                correlation_id=f"m197-realistic-corr-{index}",
            )
        )

    sync_dir = tmp_path / "sync"
    sync_result = run_universal_kb_no_write_rehearsal(sync_dir)
    projection = _load(sync_dir / "projection_result.json")
    queue = _load(sync_dir / "queue_inspect.json")

    assert len(all_events) == 12
    assert {event["job_id"] for event in all_events} == {
        "m197-realistic-job-0",
        "m197-realistic-job-1",
        "m197-realistic-job-2",
    }
    assert {event["correlation_id"] for event in all_events} == {
        "m197-realistic-corr-0",
        "m197-realistic-corr-1",
        "m197-realistic-corr-2",
    }
    assert {event["schema_version"] for event in all_events} == {contract["schema_version"]}
    for event in all_events:
        for field in contract["required_fields"]:
            assert field in event
        assert event["graph_writes_allowed"] is False
        assert event["schema_migration_allowed"] is False
        assert event["import_eligible"] is False

    completed = [event for event in all_events if event["event_type"] == "stage.completed"]
    assert len(completed) == 6
    assert {event["checksum_sha256"] for event in completed} == {"1" * 64, "2" * 64}
    assert all(event["child_artifact_refs"] for event in completed)
    assert all(event["parent_artifact_refs"] for event in completed)

    assert (sync_dir / "queue.sqlite").exists()
    assert not (sync_dir / "queue_events.json").exists()
    assert queue["job"]["job_id"] == "sidecar-candidate-1"
    assert {path.name for path in sync_result.artifact_paths} == {
        "candidate.json",
        "projection_result.json",
        "queue_inspect.json",
        "readiness_handoff.json",
        "review_packet.json",
        "review_trace.json",
        "schema_gate_result.json",
        "summary.json",
    }
    assert projection["safety_flags"]["graphdb_written"] is False
    assert projection["safety_flags"]["ladybugdb_written"] is False
    assert projection["safety_flags"]["production_import_attempted"] is False
    assert projection["safety_flags"]["graph_import_allowed"] is False
    assert projection["safety_flags"]["import_eligible"] is False


def test_realistic_no_write_rehearsal_outputs_remain_payload_safe(tmp_path: Path) -> None:
    contract = _load(CONTRACT_PATH)
    all_events = []
    for index in range(2):
        all_events.extend(
            _run_dry_job(
                tmp_path / "reactive",
                job_id=f"m197-payload-safe-job-{index}",
                correlation_id=f"m197-payload-safe-corr-{index}",
            )
        )
    sync_result = run_universal_kb_no_write_rehearsal(tmp_path / "sync")

    texts = [json.dumps(all_events)]
    texts.extend(path.read_text(encoding="utf-8") for path in sync_result.artifact_paths)
    for text in texts:
        lowered = text.lower()
        for term in contract["forbidden_payload_terms"]:
            assert term.lower() not in lowered
