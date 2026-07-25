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
async def test_reactive_stage_timeout_emits_timeout_without_completion_or_payload() -> None:
    async def stage() -> dict:
        await asyncio.sleep(0.05)
        return {"artifact_refs": ["should-not-appear.json"]}

    events = await run_reactive_stage(
        job_id="job-timeout",
        stage_id="stage.timeout",
        correlation_id="corr-timeout",
        phase="projection",
        stage=stage,
        timeout_ms=1,
    )

    assert [event["event_type"] for event in events] == ["stage.started", "stage.timeout"]
    for event in events:
        _assert_contract_event(event)
    assert events[-1]["status"] == "timeout"
    assert events[-1]["timeout_ms"] == 1
    assert events[-1]["cancelled"] is False
    assert events[-1]["diagnostics"] == {"last_error_code": "TimeoutError"}
    assert "should-not-appear" not in json.dumps(events)


@pytest.mark.asyncio
async def test_reactive_stage_cancelled_emits_cancelled_without_completion_or_payload() -> None:
    async def stage() -> dict:
        raise asyncio.CancelledError("source text should not be copied")

    events = await run_reactive_stage(
        job_id="job-cancelled",
        stage_id="stage.cancelled",
        correlation_id="corr-cancelled",
        phase="projection",
        stage=stage,
        timeout_ms=100,
    )

    assert [event["event_type"] for event in events] == ["stage.started", "stage.cancelled"]
    for event in events:
        _assert_contract_event(event)
    assert events[-1]["status"] == "cancelled"
    assert events[-1]["cancelled"] is True
    assert events[-1]["timeout_ms"] == 100
    assert events[-1]["diagnostics"] == {"last_error_code": "CancelledError"}
    assert "source text" not in json.dumps(events)


@pytest.mark.asyncio
async def test_reactive_stage_retryable_failure_emits_retryable_metadata_only() -> None:
    class RetryableProviderError(Exception):
        pass

    async def stage() -> dict:
        raise RetryableProviderError("raw prompt payload should not be copied")

    events = await run_reactive_stage(
        job_id="job-retry",
        stage_id="stage.retry",
        correlation_id="corr-retry",
        phase="llm",
        stage=stage,
        attempt=2,
        retryable_exceptions=(RetryableProviderError,),
        retry_after_ms=250,
        heartbeat_at="2026-06-30T00:00:00+00:00",
        lease_expires_at="2026-06-30T00:01:00+00:00",
    )

    assert [event["event_type"] for event in events] == ["stage.started", "stage.failed_retryable"]
    for event in events:
        _assert_contract_event(event)
        assert event["heartbeat_at"] == "2026-06-30T00:00:00+00:00"
        assert event["lease_expires_at"] == "2026-06-30T00:01:00+00:00"
    assert events[-1]["status"] == "failed_retryable"
    assert events[-1]["attempt"] == 2
    assert events[-1]["diagnostics"] == {
        "last_error_code": "RetryableProviderError",
        "retry_after_ms": 250,
    }
    assert "raw prompt payload" not in json.dumps(events)


@pytest.mark.asyncio
async def test_reactive_stage_success_emits_lineage_metadata_without_payloads() -> None:
    async def stage() -> dict:
        return {
            "artifact_refs": ["child.json"],
            "child_artifact_refs": ["child.json"],
            "checksum_sha256": "a" * 64,
            "diagnostics": {"lineage": "recorded"},
        }

    events = await run_reactive_stage(
        job_id="job-lineage",
        stage_id="stage.lineage",
        correlation_id="corr-lineage",
        phase="lineage",
        stage=stage,
        parent_artifact_refs=["parent.json"],
    )

    for event in events:
        _assert_contract_event(event)
        assert event["parent_artifact_refs"] == ["parent.json"]
    completed = events[-1]
    assert completed["event_type"] == "stage.completed"
    assert completed["child_artifact_refs"] == ["child.json"]
    assert completed["checksum_sha256"] == "a" * 64
    event_text = json.dumps(events).lower()
    for term in _contract()["forbidden_payload_terms"]:
        assert term.lower() not in event_text


@pytest.mark.asyncio
async def test_reactive_stages_bounded_forwards_parent_artifact_refs() -> None:
    async def stage() -> dict:
        return {"artifact_refs": ["child.json"], "checksum_sha256": "b" * 64}

    events = await run_reactive_stages_bounded(
        job_id="job-lineage-bounded",
        correlation_id="corr-lineage-bounded",
        max_concurrency=1,
        stages=[
            {
                "stage_id": "stage.lineage",
                "phase": "lineage",
                "stage": stage,
                "parent_artifact_refs": ["parent.json"],
            }
        ],
    )

    assert events[0]["parent_artifact_refs"] == ["parent.json"]
    assert events[-1]["child_artifact_refs"] == ["child.json"]
    assert events[-1]["checksum_sha256"] == "b" * 64
    assert events[-1]["diagnostics"]["stage_index"] == 0


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
async def test_reactive_stages_bounded_preserves_order_when_a_stage_times_out() -> None:
    async def ok_stage() -> dict:
        return {"artifact_refs": ["ok.json"], "diagnostics": {"ok": True}}

    async def slow_stage() -> dict:
        await asyncio.sleep(0.05)
        return {"artifact_refs": ["slow.json"]}

    events = await run_reactive_stages_bounded(
        job_id="job-timeout-bounded",
        correlation_id="corr-timeout-bounded",
        max_concurrency=2,
        stages=[
            {"stage_id": "stage.ok", "phase": "phase", "stage": ok_stage},
            {"stage_id": "stage.slow", "phase": "phase", "stage": slow_stage, "timeout_ms": 1},
        ],
    )

    assert [event["stage_id"] for event in events] == [
        "stage.ok",
        "stage.ok",
        "stage.slow",
        "stage.slow",
    ]
    assert [event["event_type"] for event in events] == [
        "stage.started",
        "stage.completed",
        "stage.started",
        "stage.timeout",
    ]
    assert events[-1]["diagnostics"]["stage_index"] == 1
    assert events[-1]["diagnostics"]["max_concurrency"] == 2


@pytest.mark.asyncio
async def test_reactive_stages_bounded_forwards_retry_heartbeat_and_lease_metadata() -> None:
    class RetryableProviderError(Exception):
        pass

    async def retryable_stage() -> dict:
        raise RetryableProviderError("secret_value should not be copied")

    events = await run_reactive_stages_bounded(
        job_id="job-retry-bounded",
        correlation_id="corr-retry-bounded",
        max_concurrency=1,
        stages=[
            {
                "stage_id": "stage.retry",
                "phase": "llm",
                "stage": retryable_stage,
                "retryable_exceptions": (RetryableProviderError,),
                "retry_after_ms": 500,
                "heartbeat_at": "2026-06-30T00:00:00+00:00",
                "lease_expires_at": "2026-06-30T00:02:00+00:00",
            }
        ],
    )

    assert [event["event_type"] for event in events] == ["stage.started", "stage.failed_retryable"]
    assert events[-1]["diagnostics"]["retry_after_ms"] == 500
    assert events[-1]["diagnostics"]["stage_index"] == 0
    assert events[-1]["diagnostics"]["max_concurrency"] == 1
    assert events[-1]["heartbeat_at"] == "2026-06-30T00:00:00+00:00"
    assert events[-1]["lease_expires_at"] == "2026-06-30T00:02:00+00:00"
    assert "secret_value" not in json.dumps(events)


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
