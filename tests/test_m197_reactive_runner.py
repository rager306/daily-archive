from __future__ import annotations

import asyncio
import json
from pathlib import Path

import pytest

from research_graph.workflows.universal_kb.reactive_runner import (
    run_reactive_stage,
    run_reactive_stages_bounded,
)

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "data/architecture-assessment/m197-reactive-event-contract.json"


def _contract() -> dict:
    return json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))


def _assert_contract_event(event: dict) -> None:
    contract = _contract()
    for field in contract["required_fields"]:
        assert field in event
    assert event["schema_version"] == contract["schema_version"]
    assert event["graph_writes_allowed"] is False
    assert event["schema_migration_allowed"] is False
    assert event["import_eligible"] is False


@pytest.mark.asyncio
async def test_reactive_stage_success_emits_started_and_completed_events() -> None:
    async def stage() -> dict:
        return {
            "artifact_refs": ["summary.json"],
            "diagnostics": {"schema_gate": "schema_versions_current"},
        }

    events = await run_reactive_stage(
        job_id="job-1",
        stage_id="stage.schema_gate",
        correlation_id="corr-1",
        phase="schema_gate",
        attempt=1,
        stage=stage,
    )

    assert [event["event_type"] for event in events] == ["stage.started", "stage.completed"]
    for event in events:
        _assert_contract_event(event)
        assert event["job_id"] == "job-1"
        assert event["stage_id"] == "stage.schema_gate"
        assert event["correlation_id"] == "corr-1"
        assert event["attempt"] == 1
    assert events[-1]["artifact_refs"] == ["summary.json"]
    assert events[-1]["diagnostics"] == {"schema_gate": "schema_versions_current"}


@pytest.mark.asyncio
async def test_reactive_stage_failure_emits_terminal_failure_without_payload() -> None:
    async def stage() -> dict:
        raise RuntimeError("sensitive source text should not be copied")

    events = await run_reactive_stage(
        job_id="job-2",
        stage_id="stage.failure",
        correlation_id="corr-2",
        phase="projection",
        stage=stage,
    )

    assert [event["event_type"] for event in events] == ["stage.started", "stage.failed_terminal"]
    for event in events:
        _assert_contract_event(event)
    assert events[-1]["status"] == "failed_terminal"
    assert events[-1]["diagnostics"] == {"last_error_code": "RuntimeError"}
    assert "sensitive source text" not in json.dumps(events)


@pytest.mark.asyncio
async def test_reactive_stage_accepts_sync_stage_callable_for_small_adapters() -> None:
    def stage() -> dict:
        return {"artifact_refs": ["queue_inspect.json"], "diagnostics": {"queue_status": "ready"}}

    events = await run_reactive_stage(
        job_id="job-3",
        stage_id="stage.queue",
        correlation_id="corr-3",
        phase="queue",
        stage=stage,
    )

    assert events[-1]["event_type"] == "stage.completed"
    assert events[-1]["artifact_refs"] == ["queue_inspect.json"]
    assert events[-1]["diagnostics"] == {"queue_status": "ready"}


@pytest.mark.asyncio
async def test_reactive_stages_bounded_enforces_limit_and_deterministic_order() -> None:
    active = 0
    max_seen = 0

    def make_stage(index: int):
        async def stage() -> dict:
            nonlocal active, max_seen
            active += 1
            max_seen = max(max_seen, active)
            await asyncio.sleep(0.01 * (3 - index))
            active -= 1
            return {
                "artifact_refs": [f"stage-{index}.json"],
                "diagnostics": {"completed_index": index},
            }

        return stage

    events = await run_reactive_stages_bounded(
        job_id="job-bounded",
        correlation_id="corr-bounded",
        max_concurrency=2,
        stages=[
            {"stage_id": "stage.0", "phase": "phase", "stage": make_stage(0)},
            {"stage_id": "stage.1", "phase": "phase", "stage": make_stage(1)},
            {"stage_id": "stage.2", "phase": "phase", "stage": make_stage(2)},
        ],
    )

    assert max_seen == 2
    assert [event["stage_id"] for event in events] == [
        "stage.0",
        "stage.0",
        "stage.1",
        "stage.1",
        "stage.2",
        "stage.2",
    ]
    for event in events:
        _assert_contract_event(event)
        assert event["diagnostics"]["max_concurrency"] == 2
    assert [event["artifact_refs"] for event in events if event["event_type"] == "stage.completed"] == [
        ["stage-0.json"],
        ["stage-1.json"],
        ["stage-2.json"],
    ]


@pytest.mark.asyncio
async def test_reactive_stages_bounded_rejects_invalid_concurrency() -> None:
    with pytest.raises(ValueError, match="max_concurrency"):
        await run_reactive_stages_bounded(
            job_id="job-invalid",
            correlation_id="corr-invalid",
            max_concurrency=0,
            stages=[],
        )


def test_reactive_runner_module_does_not_import_queue_or_rehearsal() -> None:
    source = (ROOT / "src/research_graph/workflows/universal_kb/reactive_runner.py").read_text(encoding="utf-8")

    assert "UniversalKBQueue" not in source
    assert "run_universal_kb_no_write_rehearsal" not in source
    assert "smoke_runner" not in source
