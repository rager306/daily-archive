"""Application-layer parser replay use case and ports.

Parser replay coordinates article selection, source loading, parsing, chunk
writing, and event emission without importing filesystem, PyMuPDF, parser, or
chunking implementations. Diagnostics are reason-coded and intentionally avoid
storing article body text.
"""

from __future__ import annotations

from collections import Counter
from collections.abc import Sequence
from dataclasses import dataclass, field
from typing import Protocol, runtime_checkable

from research_graph.domain.corpus import (
    ParserReplayArticle,
    ParserReplayChunkWriteResult,
    ParserReplayDiagnostic,
    ParserReplayDiagnosticCode,
    ParserReplayParsedArticle,
    ParserReplayRecord,
    ParserReplaySource,
    ParserReplaySourceOutcome,
    ParserReplayStatus,
)


@dataclass(frozen=True)
class ParserReplayRequest:
    """Application-level options for one parser replay run."""

    limit: int | None = None


@dataclass(frozen=True)
class ParserReplayResult:
    """Aggregate parser replay outcome with secret-free diagnostics."""

    records: list[ParserReplayRecord]
    diagnostics: list[ParserReplayDiagnostic]
    total_articles: int
    status_counts: dict[str, int] = field(default_factory=dict)
    reason_counts: dict[str, int] = field(default_factory=dict)
    total_chunks: int = 0

    @property
    def completed_count(self) -> int:
        """Number of successfully parsed and chunked articles."""

        return self.status_counts.get(ParserReplayStatus.COMPLETED.value, 0)

    @property
    def skipped_count(self) -> int:
        """Number of metadata-only or otherwise skipped articles."""

        return self.status_counts.get(ParserReplayStatus.SKIPPED.value, 0)

    @property
    def low_quality_count(self) -> int:
        """Number of sources rejected as low quality before parsing."""

        return self.status_counts.get(ParserReplayStatus.LOW_QUALITY.value, 0)

    @property
    def failed_count(self) -> int:
        """Number of parser replay failures."""

        return self.status_counts.get(ParserReplayStatus.FAILED.value, 0)

    @property
    def succeeded(self) -> bool:
        """True when no failed parser replay records were produced."""

        return self.failed_count == 0


@runtime_checkable
class ParserReplayArticleSelectorPort(Protocol):
    """Boundary for selecting catalog articles to replay."""

    def selected_articles(self) -> Sequence[ParserReplayArticle]:
        """Return catalog articles in deterministic replay order."""
        ...


@runtime_checkable
class ParserReplaySourceLoaderPort(Protocol):
    """Boundary for resolving local full-text sources."""

    def load_source(self, article: ParserReplayArticle) -> ParserReplaySourceOutcome:
        """Return source information, skip, or low-quality diagnostic outcome."""
        ...


@runtime_checkable
class FullTextParserPort(Protocol):
    """Boundary for full text parsing."""

    def parse(self, source: ParserReplaySource) -> ParserReplayParsedArticle:
        """Parse a resolved source without leaking parser implementation details."""
        ...


@runtime_checkable
class ParserReplayChunkWriterPort(Protocol):
    """Boundary for semantic chunk emission."""

    def write_chunks(
        self,
        article: ParserReplayArticle,
        source: ParserReplaySource,
        parsed: ParserReplayParsedArticle,
    ) -> ParserReplayChunkWriteResult:
        """Persist chunks and return counts/paths without chunk text payloads."""
        ...


@runtime_checkable
class ParserReplayEventSinkPort(Protocol):
    """Optional boundary for parser replay event emission."""

    def emit(self, event: dict[str, object]) -> None:
        """Emit a compact structured event."""
        ...


class ParserReplayUseCase:
    """Coordinate parser replay through application ports."""

    def __init__(
        self,
        *,
        article_selector: ParserReplayArticleSelectorPort,
        source_loader: ParserReplaySourceLoaderPort,
        full_text_parser: FullTextParserPort,
        chunk_writer: ParserReplayChunkWriterPort,
        event_sink: ParserReplayEventSinkPort | None = None,
    ) -> None:
        self._article_selector = article_selector
        self._source_loader = source_loader
        self._full_text_parser = full_text_parser
        self._chunk_writer = chunk_writer
        self._event_sink = event_sink

    def run(self, request: ParserReplayRequest | None = None) -> ParserReplayResult:
        """Run parser replay and return aggregate counts plus diagnostics."""

        req = request or ParserReplayRequest()
        articles = list(self._article_selector.selected_articles())
        if req.limit is not None:
            articles = articles[: req.limit]
        self._emit("parser_replay.started", total_articles=len(articles))

        records: list[ParserReplayRecord] = []
        diagnostics: list[ParserReplayDiagnostic] = []

        for article in articles:
            record, diagnostic = self._replay_one(article)
            records.append(record)
            if diagnostic is not None:
                diagnostics.append(diagnostic)
            self._emit(
                "parser_replay.article_completed",
                article_id=record.article_id,
                status=record.status.value,
                reason=record.reason.value if record.reason else None,
                chunk_count=record.chunk_count,
            )

        result = ParserReplayResult(
            records=records,
            diagnostics=diagnostics,
            total_articles=len(articles),
            status_counts=dict(Counter(record.status.value for record in records)),
            reason_counts=dict(
                Counter(record.reason.value for record in records if record.reason is not None)
            ),
            total_chunks=sum(record.chunk_count for record in records),
        )
        self._emit(
            "parser_replay.completed",
            total_articles=result.total_articles,
            status_counts=result.status_counts,
            reason_counts=result.reason_counts,
            total_chunks=result.total_chunks,
        )
        return result

    def _replay_one(
        self,
        article: ParserReplayArticle,
    ) -> tuple[ParserReplayRecord, ParserReplayDiagnostic | None]:
        try:
            source_outcome = self._source_loader.load_source(article)
        except Exception as exc:  # noqa: BLE001 - convert adapter failures to diagnostics
            return self._failure_record(
                article,
                phase="source_load",
                reason=ParserReplayDiagnosticCode.SOURCE_LOAD_ERROR,
                message=exc.__class__.__name__,
            )

        if source_outcome.status != ParserReplayStatus.COMPLETED:
            reason = source_outcome.reason or (
                ParserReplayDiagnosticCode.LOW_QUALITY_SOURCE
                if source_outcome.status == ParserReplayStatus.LOW_QUALITY
                else ParserReplayDiagnosticCode.METADATA_ONLY_NO_LOCAL_SOURCE
            )
            record = ParserReplayRecord(
                article_id=article.article_id,
                article_ref=article.article_ref,
                status=source_outcome.status,
                source_kind=source_outcome.source.source_kind if source_outcome.source else None,
                source_path=source_outcome.path
                or (source_outcome.source.source_path if source_outcome.source else None),
                chunk_count=0,
                reason=reason,
                message=source_outcome.message,
                text_chars=source_outcome.source.text_chars if source_outcome.source else 0,
                pdf_pages=source_outcome.source.pdf_pages if source_outcome.source else 0,
                cache_reused=source_outcome.source.cache_reused if source_outcome.source else False,
            )
            return record, ParserReplayDiagnostic(
                article_id=article.article_id,
                phase="source_load",
                reason=reason,
                message=source_outcome.message,
                path=source_outcome.path,
            )

        if source_outcome.source is None:
            return self._failure_record(
                article,
                phase="source_load",
                reason=ParserReplayDiagnosticCode.SOURCE_LOAD_ERROR,
                message="missing_source_for_completed_outcome",
                path=source_outcome.path,
            )

        source = source_outcome.source
        try:
            parsed = self._full_text_parser.parse(source)
        except Exception as exc:  # noqa: BLE001 - convert parser failures to diagnostics
            return self._failure_record(
                article,
                phase="parse",
                reason=ParserReplayDiagnosticCode.PARSE_ERROR,
                message=exc.__class__.__name__,
                source=source,
            )

        try:
            chunk_result = self._chunk_writer.write_chunks(article, source, parsed)
        except Exception as exc:  # noqa: BLE001 - convert writer failures to diagnostics
            return self._failure_record(
                article,
                phase="chunk_write",
                reason=ParserReplayDiagnosticCode.CHUNK_WRITE_ERROR,
                message=exc.__class__.__name__,
                source=source,
            )

        return ParserReplayRecord(
            article_id=article.article_id,
            article_ref=article.article_ref,
            status=ParserReplayStatus.COMPLETED,
            source_kind=source.source_kind,
            source_path=source.source_path,
            chunk_count=chunk_result.chunk_count,
            message="completed",
            text_chars=source.text_chars,
            pdf_pages=source.pdf_pages,
            cache_reused=source.cache_reused,
        ), None

    def _failure_record(
        self,
        article: ParserReplayArticle,
        *,
        phase: str,
        reason: ParserReplayDiagnosticCode,
        message: str,
        path: str | None = None,
        source: ParserReplaySource | None = None,
    ) -> tuple[ParserReplayRecord, ParserReplayDiagnostic]:
        diagnostic_path = path or (source.source_path if source else None)
        record = ParserReplayRecord(
            article_id=article.article_id,
            article_ref=article.article_ref,
            status=ParserReplayStatus.FAILED,
            source_kind=source.source_kind if source else None,
            source_path=diagnostic_path,
            chunk_count=0,
            reason=reason,
            message=message,
            text_chars=source.text_chars if source else 0,
            pdf_pages=source.pdf_pages if source else 0,
            cache_reused=source.cache_reused if source else False,
        )
        return record, ParserReplayDiagnostic(
            article_id=article.article_id,
            phase=phase,
            reason=reason,
            message=message,
            path=diagnostic_path,
        )

    def _emit(self, event: str, **payload: object) -> None:
        if self._event_sink is None:
            return
        self._event_sink.emit({"event": event, **payload})


__all__ = [
    "FullTextParserPort",
    "ParserReplayArticleSelectorPort",
    "ParserReplayChunkWriterPort",
    "ParserReplayEventSinkPort",
    "ParserReplayRequest",
    "ParserReplayResult",
    "ParserReplaySourceLoaderPort",
    "ParserReplayUseCase",
]
