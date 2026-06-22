"""Structured metadata-only logging for article ingestion load attempts.

Formerly: src/arxiv_archive/ingestion/logging.py"""

from __future__ import annotations

import json
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Protocol

from research_graph.workflows.validation.logging import ValidationLogger

PHASE = "article_loader"
LOADER_NAME = "local_article_loader"


class ArticleEventLogger(Protocol):
    """Protocol for JSON-native ingestion event sinks."""

    def emit_article_event(self, payload: dict[str, Any]) -> dict[str, Any]: ...

    def close(self) -> None: ...


class ArticleJsonlLogger:
    """Tiny JSONL sink for the article loader's flattened event contract."""

    def __init__(self, log_path: Path) -> None:
        self.log_path = Path(log_path)
        self.log_path.parent.mkdir(parents=True, exist_ok=True)

    def emit_article_event(self, payload: dict[str, Any]) -> dict[str, Any]:
        safe_payload = dict(payload)
        with self.log_path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(safe_payload, ensure_ascii=False, sort_keys=True) + "\n")
        return safe_payload

    def close(self) -> None:
        return None


def emit_load_started(
    logger: ValidationLogger | ArticleEventLogger | None,
    *,
    source_path: Path,
    paper_id: str | None,
    source_id: str,
    source_type: str,
    media_type: str,
    sha256: str | None,
    byte_size: int,
    parser_name: str,
    duration_ms: int,
) -> None:
    """Emit one metadata-only load-start event."""
    if logger is None:
        return
    payload = ingestion_event_payload(
        event="source.load_started",
        status="started",
        source_path=source_path,
        paper_id=paper_id,
        source_id=source_id,
        source_type=source_type,
        media_type=media_type,
        sha256=sha256,
        byte_size=byte_size,
        parser_name=parser_name,
        outcome="started",
        failure_reason=None,
        duration_ms=duration_ms,
        warning_count=0,
    )
    emit_payload(logger, payload)


def emit_load_terminal(
    logger: ValidationLogger | ArticleEventLogger | None,
    *,
    source_path: Path,
    paper_id: str | None,
    source_id: str,
    source_type: str,
    media_type: str,
    sha256: str | None,
    byte_size: int,
    parser_name: str,
    outcome: str,
    failure_reason: str | None,
    duration_ms: int,
    warning_count: int,
) -> None:
    """Emit exactly one metadata-only terminal event for a load attempt."""
    if logger is None:
        return
    event = (
        "source.load_completed"
        if outcome in {"loaded", "loaded_metadata_only"}
        else "source.load_failed"
    )
    status = "success" if event == "source.load_completed" else "failed"
    payload = ingestion_event_payload(
        event=event,
        status=status,
        source_path=source_path,
        paper_id=paper_id,
        source_id=source_id,
        source_type=source_type,
        media_type=media_type,
        sha256=sha256,
        byte_size=byte_size,
        parser_name=parser_name,
        outcome=outcome,
        failure_reason=failure_reason,
        duration_ms=duration_ms,
        warning_count=warning_count,
    )
    emit_payload(logger, payload)


def emit_payload(logger: ValidationLogger | ArticleEventLogger, payload: dict[str, Any]) -> None:
    """Send a flattened ingestion payload to either logging adapter."""
    if hasattr(logger, "emit_article_event"):
        logger.emit_article_event(payload)  # type: ignore[attr-defined]  # ty:ignore[call-non-callable]
        return
    details = {
        key: payload[key]
        for key in (
            "source_id",
            "source_type",
            "media_type",
            "sha256",
            "byte_size",
            "parser_name",
            "loader_name",
            "outcome",
            "failure_reason",
            "selected_fallback",
        )
    }
    # pyrefly: ignore [missing-attribute]
    logger.emit(
        payload["event"],
        phase=payload["phase"],
        status=payload["status"],
        paper_id=payload.get("paper_id"),
        source_path=payload["source_path"],
        duration_ms=payload["duration_ms"],
        warning_count=payload["warning_count"],
        details=details,
    )


def ingestion_event_payload(
    *,
    event: str,
    status: str,
    source_path: Path,
    paper_id: str | None,
    source_id: str,
    source_type: str,
    media_type: str,
    sha256: str | None,
    byte_size: int,
    parser_name: str,
    outcome: str,
    failure_reason: str | None,
    duration_ms: int,
    warning_count: int,
) -> dict[str, Any]:
    """Build the redacted structured log payload for one load event."""
    payload: dict[str, Any] = {
        "ts": datetime.now(UTC).isoformat(),
        "event": event,
        "phase": PHASE,
        "status": status,
        "source_path": str(source_path),
        "source_id": source_id,
        "source_type": source_type,
        "media_type": media_type,
        "sha256": sha256,
        "checksum": sha256,
        "byte_size": byte_size,
        "parser_name": parser_name,
        "loader_name": LOADER_NAME,
        "outcome": outcome,
        "selected_fallback": failure_reason,
        "failure_reason": failure_reason,
        "duration_ms": duration_ms,
        "warning_count": warning_count,
    }
    if paper_id is not None:
        payload["paper_id"] = paper_id
    return payload


__all__ = [
    "ArticleEventLogger",
    "ArticleJsonlLogger",
    "LOADER_NAME",
    "PHASE",
    "emit_load_started",
    "emit_load_terminal",
    "ingestion_event_payload",
]
