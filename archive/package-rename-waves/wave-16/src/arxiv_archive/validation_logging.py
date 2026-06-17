# Formerly: src/arxiv_archive/validation_logging.py

"""Structured validation logging for real-corpus KG validation runs.

This module provides a small Loguru-backed JSONL event sink for validation
scripts. It is intentionally narrow: validation logs are durable evidence for
paper-level acquisition/conversion decisions, not broad application logging.
"""

from __future__ import annotations

import json
import re
from collections.abc import Mapping
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Literal

from loguru import logger

ValidationStatus = Literal["started", "success", "failed", "skipped", "diagnostic"]

_SECRET_KEY_RE = re.compile(r"(api[_-]?key|token|secret|password|credential)", re.IGNORECASE)
_SECRET_VALUE_RE = re.compile(
    r"(?i)(sk-[A-Za-z0-9_-]{8,}|[A-Za-z0-9_]*(?:api[_-]?key|token|secret|password)[A-Za-z0-9_]*\s*[=:]\s*\S+)"
)
_MAX_ERROR_LENGTH = 300
_ALLOWED_SCALAR = str | int | float | bool | None


class ValidationLogger:
    """Write structured validation events to a JSONL file via Loguru."""

    def __init__(self, log_path: Path) -> None:
        self.log_path = Path(log_path)
        self.log_path.parent.mkdir(parents=True, exist_ok=True)
        self._sink_id = logger.add(
            self._write_event,
            level="INFO",
            filter=lambda record: "_validation_event" in record["extra"],
            catch=False,
        )

    def close(self) -> None:
        """Remove this logger's Loguru sink."""
        logger.remove(self._sink_id)

    def __enter__(self) -> ValidationLogger:
        return self

    def __exit__(self, *_exc: object) -> None:
        self.close()

    def emit(
        self,
        event: str,
        *,
        phase: str,
        status: ValidationStatus,
        paper_id: str | None = None,
        source_path: str | Path | None = None,
        output_path: str | Path | None = None,
        duration_ms: int | None = None,
        warning_count: int | None = None,
        error: BaseException | str | None = None,
        details: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Emit one validation event and return the sanitized payload."""
        payload: dict[str, Any] = {
            "ts": datetime.now(UTC).isoformat(),
            "event": event,
            "phase": phase,
            "status": status,
        }
        if paper_id is not None:
            payload["paper_id"] = paper_id
        if source_path is not None:
            payload["source_path"] = str(source_path)
        if output_path is not None:
            payload["output_path"] = str(output_path)
        if duration_ms is not None:
            payload["duration_ms"] = duration_ms
        if warning_count is not None:
            payload["warning_count"] = warning_count
        if error is not None:
            payload.update(_sanitize_error(error))
        if details is not None:
            payload["details"] = _sanitize_mapping(details)

        logger.bind(_validation_event=payload).info(event)
        return payload

    def _write_event(self, message: Any) -> None:
        event = message.record["extra"]["_validation_event"]
        with self.log_path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(event, ensure_ascii=False, sort_keys=True) + "\n")


def _sanitize_error(error: BaseException | str) -> dict[str, str]:
    if isinstance(error, BaseException):
        error_type = type(error).__name__
        message = str(error)
    else:
        error_type = "Error"
        message = error
    return {
        "error_type": error_type,
        "error_message": _redact_string(message)[:_MAX_ERROR_LENGTH],
    }


def sanitize_event_details(values: Mapping[str, Any]) -> dict[str, Any]:
    """Return a JSON-native redacted copy of event/detail metadata."""
    return _sanitize_mapping(values)


def _sanitize_mapping(values: Mapping[str, Any]) -> dict[str, Any]:
    sanitized: dict[str, Any] = {}
    for key, value in values.items():
        safe_key = str(key)
        if _SECRET_KEY_RE.search(safe_key):
            sanitized[safe_key] = "[REDACTED]"
            continue
        sanitized[safe_key] = _sanitize_value(value)
    return sanitized


def _sanitize_value(value: Any) -> Any:
    if isinstance(value, str):
        return _redact_string(value)
    if isinstance(value, Path):
        return str(value)
    if isinstance(value, _ALLOWED_SCALAR):
        return value
    if isinstance(value, Mapping):
        return _sanitize_mapping(value)
    if isinstance(value, (list, tuple)):
        return [_sanitize_value(item) for item in value[:20]]
    return repr(value)[:200]


def _redact_string(value: str) -> str:
    return _SECRET_VALUE_RE.sub("[REDACTED]", value)


__all__ = ["ValidationLogger", "ValidationStatus", "sanitize_event_details"]
