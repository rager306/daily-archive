from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from research_graph.workflows.universal_kb.rehearsal import run_universal_kb_no_write_rehearsal

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m197-reactive-event-contract.json"
SCRIPT = ROOT / "scripts/run_m197_reactive_dry_run.py"
EXPECTED_SYNC_ARTIFACTS = {
    "candidate.json",
    "projection_result.json",
    "queue_inspect.json",
    "readiness_handoff.json",
    "review_packet.json",
    "review_trace.json",
    "schema_gate_result.json",
    "summary.json",
}


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _load_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def test_reactive_dry_run_and_sync_rehearsal_preserve_no_write_surfaces(
    tmp_path: Path,
) -> None:
    dry_run_dir = tmp_path / "dry-run"
    sync_dir = tmp_path / "sync-rehearsal"
    events_path = dry_run_dir / "events.jsonl"

    subprocess.run(
        [sys.executable, str(SCRIPT), "--events", str(events_path)],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=True,
    )
    sync_result = run_universal_kb_no_write_rehearsal(sync_dir)

    events = _load_jsonl(events_path)
    queue = _load(sync_dir / "queue_inspect.json")
    projection = _load(sync_dir / "projection_result.json")
    summary = _load(sync_dir / "summary.json")

    assert len(events) == 4
    for event in events:
        assert event["graph_writes_allowed"] is False
        assert event["schema_migration_allowed"] is False
        assert event["import_eligible"] is False
    assert not (dry_run_dir / "queue.sqlite").exists()

    assert {path.name for path in sync_result.artifact_paths} == EXPECTED_SYNC_ARTIFACTS
    assert (sync_dir / "queue.sqlite").exists()
    assert not (sync_dir / "queue_events.json").exists()
    assert "job_id" in queue["job"]
    assert "id" not in queue["job"]
    assert queue["job"]["job_id"] == "sidecar-candidate-1"
    assert projection["safety_flags"]["graphdb_written"] is False
    assert projection["safety_flags"]["ladybugdb_written"] is False
    assert projection["safety_flags"]["production_import_attempted"] is False
    assert projection["safety_flags"]["graph_import_allowed"] is False
    assert projection["safety_flags"]["import_eligible"] is False
    assert summary["projection_import_eligible"] is False


def test_reactive_dry_run_payload_safety_matches_sync_baseline_terms(tmp_path: Path) -> None:
    dry_run_dir = tmp_path / "dry-run"
    sync_dir = tmp_path / "sync-rehearsal"
    events_path = dry_run_dir / "events.jsonl"
    forbidden = set(_load(CONTRACT_PATH)["forbidden_payload_terms"])

    subprocess.run(
        [sys.executable, str(SCRIPT), "--events", str(events_path)],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=True,
    )
    sync_result = run_universal_kb_no_write_rehearsal(sync_dir)

    texts = [events_path.read_text(encoding="utf-8")]
    texts.extend(path.read_text(encoding="utf-8") for path in sync_result.artifact_paths)

    for text in texts:
        lowered = text.lower()
        for term in forbidden:
            assert term.lower() not in lowered


def test_reactive_dry_run_events_keep_queue_compatibility_metadata(tmp_path: Path) -> None:
    events_path = tmp_path / "events.jsonl"

    subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--events",
            str(events_path),
            "--job-id",
            "queue-compat-job",
            "--correlation-id",
            "queue-compat-corr",
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=True,
    )

    events = _load_jsonl(events_path)
    assert [event["stage_id"] for event in events] == [
        "dry_run.schema_gate",
        "dry_run.schema_gate",
        "dry_run.projection_safety",
        "dry_run.projection_safety",
    ]
    assert events[1]["child_artifact_refs"] == ["schema_gate_result.json"]
    assert events[3]["parent_artifact_refs"] == ["schema_gate_result.json"]
    assert events[3]["child_artifact_refs"] == ["projection_safety_result.json"]
    assert {event["job_id"] for event in events} == {"queue-compat-job"}
    assert {event["correlation_id"] for event in events} == {"queue-compat-corr"}
