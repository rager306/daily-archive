from __future__ import annotations

import asyncio
import inspect
from collections.abc import Awaitable, Callable, Mapping, Sequence
from datetime import UTC, datetime
from typing import Any

SCHEMA_VERSION = "m197.reactive_event.v1"

StageCallable = Callable[[], Awaitable[Mapping[str, Any]] | Mapping[str, Any] | None]


def _utc_now() -> str:
    return datetime.now(UTC).isoformat()


def _base_event(
    *,
    event_type: str,
    job_id: str,
    stage_id: str,
    correlation_id: str,
    phase: str,
    status: str,
    attempt: int,
    artifact_refs: Sequence[str] = (),
    diagnostics: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "event_type": event_type,
        "job_id": job_id,
        "stage_id": stage_id,
        "correlation_id": correlation_id,
        "phase": phase,
        "status": status,
        "attempt": attempt,
        "timestamp": _utc_now(),
        "graph_writes_allowed": False,
        "schema_migration_allowed": False,
        "import_eligible": False,
        "artifact_refs": list(artifact_refs),
        "diagnostics": dict(diagnostics or {}),
    }


async def run_reactive_stage(
    *,
    job_id: str,
    stage_id: str,
    correlation_id: str,
    phase: str,
    stage: StageCallable,
    attempt: int = 0,
) -> list[dict[str, Any]]:
    """Run one no-write stage and return contract-shaped lifecycle events."""

    events = [
        _base_event(
            event_type="stage.started",
            job_id=job_id,
            stage_id=stage_id,
            correlation_id=correlation_id,
            phase=phase,
            status="started",
            attempt=attempt,
        )
    ]
    try:
        value = stage()
        result = await value if inspect.isawaitable(value) else value
    except Exception as exc:  # noqa: BLE001 - event boundary converts failures to metadata.
        events.append(
            _base_event(
                event_type="stage.failed_terminal",
                job_id=job_id,
                stage_id=stage_id,
                correlation_id=correlation_id,
                phase=phase,
                status="failed_terminal",
                attempt=attempt,
                diagnostics={"last_error_code": type(exc).__name__},
            )
        )
        return events

    result_mapping = dict(result or {})
    artifact_refs = result_mapping.get("artifact_refs", ())
    diagnostics = result_mapping.get("diagnostics", {})
    events.append(
        _base_event(
            event_type="stage.completed",
            job_id=job_id,
            stage_id=stage_id,
            correlation_id=correlation_id,
            phase=phase,
            status="completed",
            attempt=attempt,
            artifact_refs=artifact_refs,
            diagnostics=diagnostics,
        )
    )
    return events


async def run_reactive_stages_bounded(
    *,
    job_id: str,
    correlation_id: str,
    stages: Sequence[Mapping[str, Any]],
    max_concurrency: int,
    attempt: int = 0,
) -> list[dict[str, Any]]:
    """Run no-write stages with bounded concurrency and deterministic event ordering."""

    if max_concurrency < 1:
        raise ValueError("max_concurrency must be >= 1")

    semaphore = asyncio.Semaphore(max_concurrency)

    async def run_one(index: int, spec: Mapping[str, Any]) -> list[dict[str, Any]]:
        async with semaphore:
            events = await run_reactive_stage(
                job_id=job_id,
                stage_id=str(spec["stage_id"]),
                correlation_id=correlation_id,
                phase=str(spec["phase"]),
                stage=spec["stage"],
                attempt=attempt,
            )
        for event in events:
            event["diagnostics"].setdefault("stage_index", index)
            event["diagnostics"].setdefault("max_concurrency", max_concurrency)
        return events

    grouped_events = await asyncio.gather(
        *(run_one(index, spec) for index, spec in enumerate(stages))
    )
    return [event for events in grouped_events for event in events]
