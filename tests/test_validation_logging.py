"""Tests for Loguru-backed validation logging."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.workflows.validation.logging import ValidationLogger


def _read_events(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def test_validation_logger_writes_jsonl_event(tmp_path: Path):
    log_path = tmp_path / "validation-events.jsonl"

    with ValidationLogger(log_path) as log:
        payload = log.emit(
            "full_text.source_checked",
            phase="full_text_bridge",
            status="diagnostic",
            paper_id="2605.14995v1",
            source_path="/tmp/source.pdf",
            output_path="/tmp/full_text.md",
            duration_ms=12,
            warning_count=1,
            details={"source_exists": False, "attempt": 1},
        )

    events = _read_events(log_path)
    assert len(events) == 1
    event = events[0]
    assert event == payload
    assert event["event"] == "full_text.source_checked"
    assert event["phase"] == "full_text_bridge"
    assert event["status"] == "diagnostic"
    assert event["paper_id"] == "2605.14995v1"
    assert event["source_path"] == "/tmp/source.pdf"
    assert event["output_path"] == "/tmp/full_text.md"
    assert event["duration_ms"] == 12
    assert event["warning_count"] == 1
    assert event["details"] == {"attempt": 1, "source_exists": False}
    assert "ts" in event


def test_validation_logger_appends_events(tmp_path: Path):
    log_path = tmp_path / "validation-events.jsonl"

    with ValidationLogger(log_path) as log:
        log.emit("validation.paper_selected", phase="full_text_bridge", status="started")
        log.emit("full_text.written", phase="full_text_bridge", status="success")

    events = _read_events(log_path)
    assert [event["event"] for event in events] == [
        "validation.paper_selected",
        "full_text.written",
    ]


def test_validation_logger_redacts_secret_like_values(tmp_path: Path):
    log_path = tmp_path / "validation-events.jsonl"

    with ValidationLogger(log_path) as log:
        log.emit(
            "full_text.failed",
            phase="full_text_bridge",
            status="failed",
            error="request failed with OPENAI_API_KEY=sk-supersecret123456",
            details={
                "api_key": "sk-supersecret123456",
                "safe_count": 3,
                "nested": {"token": "abc123secret", "mode": "download"},
            },
        )

    event = _read_events(log_path)[0]
    serialized = json.dumps(event)
    assert "sk-supersecret" not in serialized
    assert "OPENAI_API_KEY" not in serialized
    assert event["error_type"] == "Error"
    assert "[REDACTED]" in event["error_message"]
    assert event["details"]["api_key"] == "[REDACTED]"
    assert event["details"]["nested"]["token"] == "[REDACTED]"
    assert event["details"]["safe_count"] == 3


def test_validation_logger_sanitizes_non_scalar_details(tmp_path: Path):
    log_path = tmp_path / "validation-events.jsonl"

    with ValidationLogger(log_path) as log:
        log.emit(
            "validation.rerun_completed",
            phase="validation_rerun",
            status="success",
            details={"path": tmp_path / "out", "items": list(range(25)), "object": object()},
        )

    details = _read_events(log_path)[0]["details"]
    assert details["path"] == str(tmp_path / "out")
    assert details["items"] == list(range(20))
    assert isinstance(details["object"], str)
