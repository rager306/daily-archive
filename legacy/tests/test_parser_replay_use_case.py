from __future__ import annotations

from collections.abc import Sequence

from research_graph.application.corpus.parser_replay import (
    ParserReplayRequest,
    ParserReplayUseCase,
)
from research_graph.domain.corpus import (
    ParserReplayArticle,
    ParserReplayChunkWriteResult,
    ParserReplayDiagnosticCode,
    ParserReplayParsedArticle,
    ParserReplaySource,
    ParserReplaySourceOutcome,
    ParserReplayStatus,
)


class FakeSelector:
    def __init__(self, articles: Sequence[ParserReplayArticle]) -> None:
        self.articles = list(articles)

    def selected_articles(self) -> Sequence[ParserReplayArticle]:
        return self.articles


class FakeSourceLoader:
    def __init__(self, outcomes: dict[str, ParserReplaySourceOutcome]) -> None:
        self.outcomes = outcomes

    def load_source(self, article: ParserReplayArticle) -> ParserReplaySourceOutcome:
        return self.outcomes[article.article_id]


class ExplodingSourceLoader:
    def load_source(self, article: ParserReplayArticle) -> ParserReplaySourceOutcome:
        raise RuntimeError("source body text must not leak")


class FakeParser:
    def __init__(self, *, fail: bool = False) -> None:
        self.fail = fail
        self.parsed_sources: list[str] = []

    def parse(self, source: ParserReplaySource) -> ParserReplayParsedArticle:
        self.parsed_sources.append(source.source_path)
        if self.fail:
            raise RuntimeError("RAW BODY TEXT SHOULD NOT LEAK")
        return ParserReplayParsedArticle(
            article_id=source.article_id,
            payload={"parsed": source.article_id},
            section_count=2,
            parser_warnings=[],
        )


class FakeChunkWriter:
    def __init__(self, chunk_count: int = 3) -> None:
        self.chunk_count = chunk_count
        self.writes: list[str] = []

    def write_chunks(
        self,
        article: ParserReplayArticle,
        source: ParserReplaySource,
        parsed: ParserReplayParsedArticle,
    ) -> ParserReplayChunkWriteResult:
        self.writes.append(parsed.article_id)
        return ParserReplayChunkWriteResult(
            chunk_count=self.chunk_count,
            output_paths=[f"chunks/{article.article_id}.json"],
            warnings=[],
        )


class FakeEventSink:
    def __init__(self) -> None:
        self.events: list[dict[str, object]] = []

    def emit(self, event: dict[str, object]) -> None:
        self.events.append(event)


def _article(article_id: str = "2605.18747") -> ParserReplayArticle:
    return ParserReplayArticle(
        article_id=article_id,
        article_ref=f"arxiv/cs-lg/{article_id}",
        article_path=f"article_catalog/arxiv/cs-lg/{article_id}/article.json",
    )


def _source(article_id: str = "2605.18747") -> ParserReplaySource:
    return ParserReplaySource(
        article_id=article_id,
        source_kind="pdf_converted",
        source_path=f"cache/{article_id}.txt",
        text_chars=2048,
        cache_reused=True,
        pdf_pages=7,
    )


def test_parser_replay_use_case_success_records_counts_and_events() -> None:
    article = _article()
    source = _source()
    event_sink = FakeEventSink()
    parser = FakeParser()
    writer = FakeChunkWriter(chunk_count=4)

    result = ParserReplayUseCase(
        article_selector=FakeSelector([article]),
        source_loader=FakeSourceLoader(
            {article.article_id: ParserReplaySourceOutcome(ParserReplayStatus.COMPLETED, source)}
        ),
        full_text_parser=parser,
        chunk_writer=writer,
        event_sink=event_sink,
    ).run()

    assert result.succeeded is True
    assert result.total_articles == 1
    assert result.status_counts == {ParserReplayStatus.COMPLETED.value: 1}
    assert result.reason_counts == {}
    assert result.completed_count == 1
    assert result.total_chunks == 4
    assert result.diagnostics == []
    assert parser.parsed_sources == [source.source_path]
    assert writer.writes == [article.article_id]
    assert event_sink.events[0]["event"] == "parser_replay.started"
    assert event_sink.events[-1]["total_chunks"] == 4


def test_parser_replay_use_case_metadata_only_skip_is_typed_diagnostic() -> None:
    article = _article("metadata-only")

    result = ParserReplayUseCase(
        article_selector=FakeSelector([article]),
        source_loader=FakeSourceLoader(
            {
                article.article_id: ParserReplaySourceOutcome(
                    status=ParserReplayStatus.SKIPPED,
                    reason=ParserReplayDiagnosticCode.METADATA_ONLY_NO_LOCAL_SOURCE,
                    message="metadata_only_no_local_source_artifact",
                    path="article.json",
                )
            }
        ),
        full_text_parser=FakeParser(),
        chunk_writer=FakeChunkWriter(),
    ).run()

    assert result.succeeded is True
    assert result.skipped_count == 1
    assert result.reason_counts == {
        ParserReplayDiagnosticCode.METADATA_ONLY_NO_LOCAL_SOURCE.value: 1
    }
    assert result.records[0].status == ParserReplayStatus.SKIPPED
    assert result.records[0].reason == ParserReplayDiagnosticCode.METADATA_ONLY_NO_LOCAL_SOURCE
    assert result.records[0].cache_reused is False
    assert result.diagnostics[0].reason == ParserReplayDiagnosticCode.METADATA_ONLY_NO_LOCAL_SOURCE
    assert result.total_chunks == 0


def test_parser_replay_use_case_defaults_missing_skip_reason() -> None:
    article = _article("metadata-only-no-reason")

    result = ParserReplayUseCase(
        article_selector=FakeSelector([article]),
        source_loader=FakeSourceLoader(
            {
                article.article_id: ParserReplaySourceOutcome(
                    status=ParserReplayStatus.SKIPPED,
                    message="metadata_only_no_local_source_artifact",
                    path="article.json",
                )
            }
        ),
        full_text_parser=FakeParser(),
        chunk_writer=FakeChunkWriter(),
    ).run()

    assert result.succeeded is True
    assert result.records[0].reason == ParserReplayDiagnosticCode.METADATA_ONLY_NO_LOCAL_SOURCE
    assert result.diagnostics[0].reason == ParserReplayDiagnosticCode.METADATA_ONLY_NO_LOCAL_SOURCE
    assert result.reason_counts == {
        ParserReplayDiagnosticCode.METADATA_ONLY_NO_LOCAL_SOURCE.value: 1
    }


def test_parser_replay_use_case_low_quality_source_is_not_parsed() -> None:
    article = _article("low-quality")
    parser = FakeParser()
    writer = FakeChunkWriter()

    result = ParserReplayUseCase(
        article_selector=FakeSelector([article]),
        source_loader=FakeSourceLoader(
            {
                article.article_id: ParserReplaySourceOutcome(
                    status=ParserReplayStatus.LOW_QUALITY,
                    reason=ParserReplayDiagnosticCode.LOW_QUALITY_SOURCE,
                    message="fallback_reason=no_substantive_body",
                    path="cache/low-quality.txt",
                )
            }
        ),
        full_text_parser=parser,
        chunk_writer=writer,
    ).run()

    assert result.succeeded is True
    assert result.low_quality_count == 1
    assert result.reason_counts == {ParserReplayDiagnosticCode.LOW_QUALITY_SOURCE.value: 1}
    assert parser.parsed_sources == []
    assert writer.writes == []
    assert result.records[0].status == ParserReplayStatus.LOW_QUALITY
    assert result.records[0].cache_reused is False
    assert result.diagnostics[0].path == "cache/low-quality.txt"


def test_parser_replay_use_case_low_quality_source_uses_source_diagnostics() -> None:
    article = _article("low-quality-with-source")
    source = _source(article.article_id)
    parser = FakeParser()
    writer = FakeChunkWriter()

    result = ParserReplayUseCase(
        article_selector=FakeSelector([article]),
        source_loader=FakeSourceLoader(
            {
                article.article_id: ParserReplaySourceOutcome(
                    status=ParserReplayStatus.LOW_QUALITY,
                    source=source,
                    message="fallback_reason=no_substantive_body",
                )
            }
        ),
        full_text_parser=parser,
        chunk_writer=writer,
    ).run()

    assert result.succeeded is True
    assert result.records[0].reason == ParserReplayDiagnosticCode.LOW_QUALITY_SOURCE
    assert result.records[0].source_path == source.source_path
    assert result.records[0].source_kind == source.source_kind
    assert result.records[0].text_chars == source.text_chars
    assert result.records[0].pdf_pages == source.pdf_pages
    assert result.records[0].cache_reused is True
    assert result.diagnostics[0].path is None
    assert parser.parsed_sources == []
    assert writer.writes == []


def test_parser_replay_use_case_source_loader_errors_are_sanitized() -> None:
    article = _article("source-error")

    result = ParserReplayUseCase(
        article_selector=FakeSelector([article]),
        source_loader=ExplodingSourceLoader(),
        full_text_parser=FakeParser(),
        chunk_writer=FakeChunkWriter(),
    ).run()

    assert result.succeeded is False
    assert result.failed_count == 1
    assert result.records[0].reason == ParserReplayDiagnosticCode.SOURCE_LOAD_ERROR
    assert result.records[0].source_path is None
    assert result.records[0].text_chars == 0
    assert result.records[0].pdf_pages == 0
    assert result.records[0].cache_reused is False
    assert result.diagnostics[0].message == "RuntimeError"
    assert "source body text" not in repr(result.records)
    assert "source body text" not in repr(result.diagnostics)


def test_parser_replay_use_case_parser_error_accounting_avoids_body_text() -> None:
    article = _article("parse-error")
    source = _source(article.article_id)

    result = ParserReplayUseCase(
        article_selector=FakeSelector([article]),
        source_loader=FakeSourceLoader(
            {article.article_id: ParserReplaySourceOutcome(ParserReplayStatus.COMPLETED, source)}
        ),
        full_text_parser=FakeParser(fail=True),
        chunk_writer=FakeChunkWriter(),
    ).run()

    assert result.succeeded is False
    assert result.failed_count == 1
    assert result.status_counts == {ParserReplayStatus.FAILED.value: 1}
    assert result.reason_counts == {ParserReplayDiagnosticCode.PARSE_ERROR.value: 1}
    assert result.records[0].reason == ParserReplayDiagnosticCode.PARSE_ERROR
    assert result.records[0].message == "RuntimeError"
    assert result.records[0].source_path == source.source_path
    assert result.records[0].cache_reused is True
    assert result.diagnostics[0].message == "RuntimeError"
    assert result.diagnostics[0].path == source.source_path
    assert "RAW BODY TEXT" not in repr(result.records)
    assert "RAW BODY TEXT" not in repr(result.diagnostics)


def test_parser_replay_request_limit_bounds_selected_articles() -> None:
    articles = [_article("a1"), _article("a2")]
    outcomes = {
        article.article_id: ParserReplaySourceOutcome(
            ParserReplayStatus.COMPLETED,
            _source(article.article_id),
        )
        for article in articles
    }

    result = ParserReplayUseCase(
        article_selector=FakeSelector(articles),
        source_loader=FakeSourceLoader(outcomes),
        full_text_parser=FakeParser(),
        chunk_writer=FakeChunkWriter(chunk_count=1),
    ).run(ParserReplayRequest(limit=1))

    assert result.total_articles == 1
    assert [record.article_id for record in result.records] == ["a1"]
    assert result.total_chunks == 1
