"""Local article source loading boundary with provenance-first diagnostics.

The article loader is intentionally local-only.  It classifies existing source
artifacts, reads text-like payloads when safe, computes deterministic provenance,
and emits metadata-only JSONL events for downstream evidence-bundle work.  It
never performs acquisition, conversion, graph imports, embeddings, vector-store
writes, or network calls.
"""

from __future__ import annotations

import hashlib
import json
import time
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Literal, Protocol

from arxiv_archive.full_text import FullTextQualityReport, assess_full_text_quality
from arxiv_archive.validation_logging import ValidationLogger

ArticleSourceType = Literal["auto", "markdown", "html", "pdf", "text", "ocr"]
ArticleOutcome = Literal["loaded", "loaded_metadata_only", "failed"]

_PHASE = "article_loader"
_LOADER_NAME = "local_article_loader"
_DEFAULT_MEDIA_TYPE = "application/octet-stream"


class _ArticleEventLogger(Protocol):
    def emit_article_event(self, payload: dict[str, Any]) -> dict[str, Any]: ...

    def close(self) -> None: ...


class _ArticleJsonlLogger:
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


@dataclass(frozen=True)
class ArticleLoadSource:
    """Caller-supplied local source descriptor for one article artifact."""

    source_path: Path
    paper_id: str | None = None
    source_type: str = "auto"

    def __post_init__(self) -> None:
        object.__setattr__(self, "source_path", Path(self.source_path))


@dataclass(frozen=True)
class ArticleSourceMetadata:
    """Deterministic source classification and checksum metadata."""

    source_path: Path
    source_type: str
    media_type: str
    sha256: str | None
    byte_size: int
    source_id: str
    parser_name: str
    paper_id: str | None = None


@dataclass(frozen=True)
class ArticleLoadResult:
    """Article loader result containing provenance and optional text payload."""

    source_path: Path
    source_type: str
    media_type: str
    sha256: str | None
    byte_size: int
    source_id: str
    parser_name: str
    loader_name: str
    outcome: str
    failure_reason: str | None
    warnings: list[str]
    duration_ms: int
    paper_id: str | None = None
    text: str | None = None
    quality: FullTextQualityReport | None = None
    provenance: dict[str, str | int | None] | None = None

    @property
    def warning_count(self) -> int:
        """Return the machine-readable warning count used in events."""
        return len(self.warnings)


@dataclass(frozen=True)
class _Classification:
    source_path: Path
    source_type: str
    media_type: str
    parser_name: str
    sha256: str | None
    byte_size: int
    source_id: str
    paper_id: str | None = None


def classify_article_source(
    source: str | Path | ArticleLoadSource,
    *,
    source_type: ArticleSourceType | str = "auto",
    paper_id: str | None = None,
) -> ArticleSourceMetadata:
    """Classify a local article source and return deterministic metadata.

    Classification is based on caller-explicit source type first, then file
    extension and lightweight content signatures.  Existing files are read once
    for checksum and signature sniffing; missing paths produce stable missing
    metadata without raising.
    """
    descriptor = _coerce_source(source, source_type=source_type, paper_id=paper_id)
    raw_bytes = descriptor.source_path.read_bytes() if descriptor.source_path.exists() else None
    classified = _classify_from_bytes(descriptor, raw_bytes)
    return ArticleSourceMetadata(**classified.__dict__)


def load_article_source(
    source: str | Path | ArticleLoadSource,
    *,
    logger: ValidationLogger | _ArticleEventLogger | None = None,
    log_path: str | Path | None = None,
    source_type: ArticleSourceType | str = "auto",
    paper_id: str | None = None,
) -> ArticleLoadResult:
    """Load one local article source and emit redacted metadata-only events.

    Text-like Markdown, HTML, OCR, and plain text sources return in-memory text
    when UTF-8 decoding and quality checks pass.  PDF inputs are checksummed and
    classified as metadata-only.  All failure modes return typed results and a
    single terminal event rather than leaking exceptions or raw payloads.
    """
    start = time.perf_counter()
    owns_logger = logger is None and log_path is not None
    active_logger = logger or (_ArticleJsonlLogger(Path(log_path)) if log_path is not None else None)

    try:
        descriptor = _coerce_source(source, source_type=source_type, paper_id=paper_id)
        raw_bytes = _read_source_bytes(descriptor.source_path)
        classified = _classify_from_bytes(descriptor, raw_bytes)
        _emit_event(active_logger, "source.load_started", classified, start, outcome="started")

        if raw_bytes is None:
            result = _build_result(
                classified=classified,
                start=start,
                outcome="failed",
                failure_reason="source_missing",
                warnings=["source path does not exist"],
            )
            _emit_terminal(active_logger, result)
            return result

        if classified.source_type == "unsupported":
            suffix = classified.source_path.suffix.lower() or "<none>"
            result = _build_result(
                classified=classified,
                start=start,
                outcome="failed",
                failure_reason="unsupported_type",
                warnings=[f"unsupported source extension: {suffix}"],
            )
            _emit_terminal(active_logger, result)
            return result

        if classified.source_type == "pdf":
            result = _build_result(
                classified=classified,
                start=start,
                outcome="loaded_metadata_only",
                failure_reason=None,
                warnings=[],
            )
            _emit_terminal(active_logger, result)
            return result

        text = _decode_text(raw_bytes)
        if text is None:
            result = _build_result(
                classified=classified,
                start=start,
                outcome="failed",
                failure_reason="decode_failed",
                warnings=["source could not be decoded as utf-8 text"],
            )
            _emit_terminal(active_logger, result)
            return result

        stripped = text.strip()
        if not stripped:
            result = _build_result(
                classified=classified,
                start=start,
                outcome="failed",
                failure_reason="source_empty",
                warnings=["source file is empty after trimming whitespace"],
            )
            _emit_terminal(active_logger, result)
            return result

        quality = _assess_quality_if_applicable(classified.source_type, stripped)
        if quality is not None and quality.status == "no_substantive_body":
            result = _build_result(
                classified=classified,
                start=start,
                outcome="failed",
                failure_reason=quality.fallback_reason,
                warnings=quality.warnings,
                quality=quality,
            )
            _emit_terminal(active_logger, result)
            return result

        result = _build_result(
            classified=classified,
            start=start,
            outcome="loaded",
            failure_reason=None,
            warnings=[],
            text=stripped,
            quality=quality,
        )
        _emit_terminal(active_logger, result)
        return result
    finally:
        if owns_logger and active_logger is not None:
            active_logger.close()


def _coerce_source(
    source: str | Path | ArticleLoadSource,
    *,
    source_type: ArticleSourceType | str,
    paper_id: str | None,
) -> ArticleLoadSource:
    if isinstance(source, ArticleLoadSource):
        explicit_type = source.source_type if source_type == "auto" else source_type
        explicit_paper_id = source.paper_id if paper_id is None else paper_id
        return ArticleLoadSource(source.source_path, paper_id=explicit_paper_id, source_type=explicit_type)
    return ArticleLoadSource(Path(source), paper_id=paper_id, source_type=source_type)


def _read_source_bytes(source_path: Path) -> bytes | None:
    if not source_path.exists():
        return None
    return source_path.read_bytes()


def _classify_from_bytes(source: ArticleLoadSource, raw_bytes: bytes | None) -> _Classification:
    source_path = source.source_path
    sha256 = hashlib.sha256(raw_bytes).hexdigest() if raw_bytes is not None else None
    byte_size = len(raw_bytes) if raw_bytes is not None else 0
    explicit_type = source.source_type.lower()
    source_type = _sniff_source_type(source_path, raw_bytes, explicit_type)
    media_type, parser_name = _type_metadata(source_type)
    source_id = _source_id(source_path=source_path, sha256=sha256, source_type=source_type)
    return _Classification(
        source_path=source_path,
        source_type=source_type,
        media_type=media_type,
        parser_name=parser_name,
        sha256=sha256,
        byte_size=byte_size,
        source_id=source_id,
        paper_id=source.paper_id,
    )


def _sniff_source_type(source_path: Path, raw_bytes: bytes | None, explicit_type: str) -> str:
    if raw_bytes is None:
        return "unknown"
    if explicit_type in {"markdown", "html", "pdf", "text"}:
        return explicit_type
    if explicit_type == "ocr":
        return "text"
    if raw_bytes.startswith(b"%PDF-"):
        return "pdf"

    suffix = source_path.suffix.lower()
    if suffix == ".md":
        return "markdown"
    if suffix in {".html", ".htm"}:
        return "html"
    if suffix == ".pdf":
        return "pdf"
    if suffix == ".txt":
        return "text"
    return "unsupported"


def _type_metadata(source_type: str) -> tuple[str, str]:
    if source_type == "markdown":
        return "text/markdown", "markdown_loader"
    if source_type == "html":
        return "text/html", "html_loader"
    if source_type == "pdf":
        return "application/pdf", "pdf_metadata_probe"
    if source_type == "text":
        return "text/plain", "text_loader"
    if source_type == "unsupported":
        return _DEFAULT_MEDIA_TYPE, "unsupported_loader"
    return _DEFAULT_MEDIA_TYPE, "unknown_loader"


def _source_id(*, source_path: Path, sha256: str | None, source_type: str) -> str:
    identity = sha256 if sha256 is not None else f"missing:{source_path}"
    digest = hashlib.sha256(f"{source_type}:{source_path}:{identity}".encode("utf-8")).hexdigest()
    return f"article-source:{digest[:24]}"


def _decode_text(raw_bytes: bytes) -> str | None:
    try:
        return raw_bytes.decode("utf-8")
    except UnicodeDecodeError:
        return None


def _assess_quality_if_applicable(source_type: str, text: str) -> FullTextQualityReport | None:
    if source_type in {"markdown", "text"}:
        return assess_full_text_quality(text)
    return None


def _build_result(
    *,
    classified: _Classification,
    start: float,
    outcome: str,
    failure_reason: str | None,
    warnings: list[str],
    text: str | None = None,
    quality: FullTextQualityReport | None = None,
) -> ArticleLoadResult:
    duration_ms = _duration_ms(start)
    provenance: dict[str, str | int | None] = {
        "source_id": classified.source_id,
        "source_path": str(classified.source_path),
        "source_type": classified.source_type,
        "media_type": classified.media_type,
        "sha256": classified.sha256,
        "byte_size": classified.byte_size,
        "parser_name": classified.parser_name,
        "loader_name": _LOADER_NAME,
    }
    if classified.paper_id is not None:
        provenance["paper_id"] = classified.paper_id
    return ArticleLoadResult(
        source_path=classified.source_path,
        source_type=classified.source_type,
        media_type=classified.media_type,
        sha256=classified.sha256,
        byte_size=classified.byte_size,
        source_id=classified.source_id,
        parser_name=classified.parser_name,
        loader_name=_LOADER_NAME,
        outcome=outcome,
        failure_reason=failure_reason,
        warnings=warnings,
        duration_ms=duration_ms,
        paper_id=classified.paper_id,
        text=text,
        quality=quality,
        provenance=provenance,
    )


def _emit_terminal(logger: ValidationLogger | None, result: ArticleLoadResult) -> None:
    event = "source.load_completed" if result.outcome in {"loaded", "loaded_metadata_only"} else "source.load_failed"
    _emit_result_event(logger, event, result)


def _emit_event(
    logger: ValidationLogger | _ArticleEventLogger | None,
    event: str,
    classified: _Classification,
    start: float,
    *,
    outcome: str,
) -> None:
    if logger is None:
        return
    payload = _event_payload(
        event=event,
        status="started",
        phase=_PHASE,
        source_path=classified.source_path,
        paper_id=classified.paper_id,
        source_id=classified.source_id,
        source_type=classified.source_type,
        media_type=classified.media_type,
        sha256=classified.sha256,
        byte_size=classified.byte_size,
        parser_name=classified.parser_name,
        loader_name=_LOADER_NAME,
        outcome=outcome,
        failure_reason=None,
        duration_ms=_duration_ms(start),
        warning_count=0,
    )
    _emit_payload(logger, payload)


def _emit_result_event(
    logger: ValidationLogger | _ArticleEventLogger | None, event: str, result: ArticleLoadResult
) -> None:
    if logger is None:
        return
    status = "success" if event == "source.load_completed" else "failed"
    payload = _event_payload(
        event=event,
        status=status,
        phase=_PHASE,
        source_path=result.source_path,
        paper_id=result.paper_id,
        source_id=result.source_id,
        source_type=result.source_type,
        media_type=result.media_type,
        sha256=result.sha256,
        byte_size=result.byte_size,
        parser_name=result.parser_name,
        loader_name=result.loader_name,
        outcome=result.outcome,
        failure_reason=result.failure_reason,
        duration_ms=result.duration_ms,
        warning_count=result.warning_count,
    )
    _emit_payload(logger, payload)


def _emit_payload(logger: ValidationLogger | _ArticleEventLogger, payload: dict[str, Any]) -> None:
    if hasattr(logger, "emit_article_event"):
        logger.emit_article_event(payload)
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
        )
    }
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


def _event_payload(
    *,
    event: str,
    status: str,
    phase: str,
    source_path: Path,
    paper_id: str | None,
    source_id: str,
    source_type: str,
    media_type: str,
    sha256: str | None,
    byte_size: int,
    parser_name: str,
    loader_name: str,
    outcome: str,
    failure_reason: str | None,
    duration_ms: int,
    warning_count: int,
) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "ts": datetime.now(UTC).isoformat(),
        "event": event,
        "phase": phase,
        "status": status,
        "source_path": str(source_path),
        "source_id": source_id,
        "source_type": source_type,
        "media_type": media_type,
        "sha256": sha256,
        "byte_size": byte_size,
        "parser_name": parser_name,
        "loader_name": loader_name,
        "outcome": outcome,
        "failure_reason": failure_reason,
        "duration_ms": duration_ms,
        "warning_count": warning_count,
    }
    if paper_id is not None:
        payload["paper_id"] = paper_id
    return payload


def _event_details(
    *,
    source_id: str,
    source_type: str,
    media_type: str,
    sha256: str | None,
    byte_size: int,
    parser_name: str,
    loader_name: str,
    outcome: str,
    failure_reason: str | None,
) -> dict[str, Any]:
    return {
        "source_id": source_id,
        "source_type": source_type,
        "media_type": media_type,
        "sha256": sha256,
        "byte_size": byte_size,
        "parser_name": parser_name,
        "loader_name": loader_name,
        "outcome": outcome,
        "failure_reason": failure_reason,
    }


def _duration_ms(start: float) -> int:
    return max(0, int((time.perf_counter() - start) * 1000))


__all__ = [
    "ArticleLoadResult",
    "ArticleLoadSource",
    "ArticleOutcome",
    "ArticleSourceMetadata",
    "ArticleSourceType",
    "classify_article_source",
    "load_article_source",
]
