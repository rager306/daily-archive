"""Contract tests for the S01 local article loader boundary.

These tests define the public article-loader API before implementation.  The
loader is intentionally local-only: it classifies and reads existing artifacts,
emits redacted JSONL events, and never performs acquisition, graph imports,
embedding generation, or raw-payload logging.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

import pytest

from arxiv_archive.article_loader import (
    ArticleLoadSource,
    classify_article_source,
    load_article_source,
)
from arxiv_archive.full_text import (
    FullTextSource,
    assess_full_text_quality,
    ingest_full_text,
)

FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_loader"


TEXT_LIKE_CASES = [
    ("structured_paper.md", "markdown", "text/markdown", "markdown_loader"),
    ("minimal_article.html", "html", "text/html", "html_loader"),
    ("ocr_text.txt", "text", "text/plain", "text_loader"),
]


FORBIDDEN_LOG_SNIPPETS = [
    "%PDF-1.4",
    "This HTML article fixture exercises deterministic local source classification.",
    "Local article loading provides a reliable contract",
    "OPENAI_API_KEY",
    "sk-test-secret",
    "base64",
    "embedding",
    "embeddings",
    "vector",
    "vectors",
    "token",
    "api_key",
]


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _read_events(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def _assert_common_metadata(record: object, source_path: Path, source_type: str, media_type: str) -> None:
    assert record.source_path == source_path
    assert record.source_type == source_type
    assert record.media_type == media_type
    assert record.sha256 == _sha256(source_path)
    assert record.byte_size == source_path.stat().st_size
    assert record.source_id
    assert record.source_id.startswith("article-source:")


def _assert_safe_event_payload(events: list[dict]) -> None:
    serialized = json.dumps(events, sort_keys=True)
    for forbidden in FORBIDDEN_LOG_SNIPPETS:
        assert forbidden not in serialized
    assert "raw_text" not in serialized
    assert "raw_bytes" not in serialized
    assert "binary_payload" not in serialized


@pytest.mark.parametrize(("fixture_name", "source_type", "media_type", "parser_name"), TEXT_LIKE_CASES)
def test_classifies_text_like_article_sources_with_deterministic_metadata(
    fixture_name: str,
    source_type: str,
    media_type: str,
    parser_name: str,
) -> None:
    source_path = FIXTURES_DIR / fixture_name

    first = classify_article_source(source_path)
    second = classify_article_source(source_path)

    _assert_common_metadata(first, source_path, source_type, media_type)
    assert first.parser_name == parser_name
    assert first.source_id == second.source_id
    assert first.sha256 == second.sha256
    assert first.byte_size == second.byte_size


def test_loads_markdown_with_text_payload_provenance_and_completed_events(tmp_path: Path) -> None:
    source_path = FIXTURES_DIR / "structured_paper.md"
    log_path = tmp_path / "article-loader.jsonl"

    result = load_article_source(source_path, log_path=log_path)

    _assert_common_metadata(result, source_path, "markdown", "text/markdown")
    assert result.parser_name == "markdown_loader"
    assert result.loader_name == "local_article_loader"
    assert result.outcome == "loaded"
    assert result.failure_reason is None
    assert result.warning_count == 0
    assert result.warnings == []
    assert result.text is not None
    assert "Graph-Guided Retrieval for Scientific Agents" in result.text
    assert result.provenance == {
        "source_id": result.source_id,
        "source_path": str(source_path),
        "source_type": "markdown",
        "media_type": "text/markdown",
        "sha256": result.sha256,
        "byte_size": result.byte_size,
        "parser_name": "markdown_loader",
        "loader_name": "local_article_loader",
    }

    events = _read_events(log_path)
    assert [event["event"] for event in events] == ["source.load_started", "source.load_completed"]
    for event in events:
        assert event["phase"] == "article_loader"
        assert event["source_path"] == str(source_path)
        assert event["source_id"] == result.source_id
        assert event["source_type"] == "markdown"
        assert event["media_type"] == "text/markdown"
        assert event["sha256"] == result.sha256
        assert event["byte_size"] == result.byte_size
        assert event["loader_name"] == "local_article_loader"
        assert event["parser_name"] == "markdown_loader"
        assert "duration_ms" in event
        assert event["warning_count"] == 0
    assert events[-1]["outcome"] == "loaded"
    assert events[-1]["failure_reason"] is None
    _assert_safe_event_payload(events)


@pytest.mark.parametrize(("fixture_name", "source_type", "media_type", "parser_name"), TEXT_LIKE_CASES[1:])
def test_loads_html_and_ocr_text_as_text_like_sources(
    tmp_path: Path,
    fixture_name: str,
    source_type: str,
    media_type: str,
    parser_name: str,
) -> None:
    source_path = FIXTURES_DIR / fixture_name
    result = load_article_source(source_path, log_path=tmp_path / f"{source_type}.jsonl")

    _assert_common_metadata(result, source_path, source_type, media_type)
    assert result.parser_name == parser_name
    assert result.outcome == "loaded"
    assert result.failure_reason is None
    assert result.text is not None
    assert len(result.text.strip()) > 40


def test_pdf_fixture_is_classified_without_binary_text_payload_or_raw_log_bytes(tmp_path: Path) -> None:
    source_path = FIXTURES_DIR / "minimal.pdf"
    log_path = tmp_path / "pdf-load.jsonl"

    result = load_article_source(source_path, log_path=log_path)

    _assert_common_metadata(result, source_path, "pdf", "application/pdf")
    assert result.parser_name == "pdf_metadata_probe"
    assert result.outcome == "loaded_metadata_only"
    assert result.failure_reason is None
    assert result.text is None
    assert result.warning_count == 0

    events = _read_events(log_path)
    assert [event["event"] for event in events] == ["source.load_started", "source.load_completed"]
    assert events[-1]["outcome"] == "loaded_metadata_only"
    _assert_safe_event_payload(events)


def test_missing_source_returns_typed_failure_and_failed_event(tmp_path: Path) -> None:
    source_path = tmp_path / "missing.md"
    log_path = tmp_path / "missing.jsonl"

    result = load_article_source(source_path, log_path=log_path)

    assert result.source_path == source_path
    assert result.source_type == "unknown"
    assert result.media_type == "application/octet-stream"
    assert result.sha256 is None
    assert result.byte_size == 0
    assert result.source_id.startswith("article-source:")
    assert result.outcome == "failed"
    assert result.failure_reason == "source_missing"
    assert result.text is None
    assert result.warning_count == 1
    assert result.warnings == ["source path does not exist"]

    events = _read_events(log_path)
    assert [event["event"] for event in events] == ["source.load_started", "source.load_failed"]
    assert events[-1]["phase"] == "article_loader"
    assert events[-1]["outcome"] == "failed"
    assert events[-1]["failure_reason"] == "source_missing"
    assert events[-1]["warning_count"] == 1
    _assert_safe_event_payload(events)


def test_empty_source_returns_typed_failure_without_silent_empty_text(tmp_path: Path) -> None:
    source_path = tmp_path / "empty.md"
    source_path.write_text("  \n\n", encoding="utf-8")

    result = load_article_source(source_path, log_path=tmp_path / "empty.jsonl")

    _assert_common_metadata(result, source_path, "markdown", "text/markdown")
    assert result.outcome == "failed"
    assert result.failure_reason == "source_empty"
    assert result.text is None
    assert result.warning_count == 1
    assert result.warnings == ["source file is empty after trimming whitespace"]


def test_unsupported_extension_returns_typed_failure_before_parsing(tmp_path: Path) -> None:
    source_path = tmp_path / "paper.xyz"
    source_path.write_text("content that should not be parsed", encoding="utf-8")

    result = load_article_source(source_path, log_path=tmp_path / "unsupported.jsonl")

    _assert_common_metadata(result, source_path, "unsupported", "application/octet-stream")
    assert result.parser_name == "unsupported_loader"
    assert result.outcome == "failed"
    assert result.failure_reason == "unsupported_type"
    assert result.text is None
    assert result.warning_count == 1
    assert result.warnings == ["unsupported source extension: .xyz"]


def test_binary_bytes_under_text_extension_returns_decode_failure(tmp_path: Path) -> None:
    source_path = tmp_path / "binary.md"
    source_path.write_bytes(b"\xff\xfe\x00not utf-8 markdown\x80")

    result = load_article_source(source_path, log_path=tmp_path / "decode.jsonl")

    _assert_common_metadata(result, source_path, "markdown", "text/markdown")
    assert result.outcome == "failed"
    assert result.failure_reason == "decode_failed"
    assert result.text is None
    assert result.warning_count == 1
    assert result.warnings == ["source could not be decoded as utf-8 text"]


def test_low_quality_markdown_reuses_full_text_quality_contract(tmp_path: Path) -> None:
    source_path = FIXTURES_DIR / "arxiv_landing_only.md"
    expected_ingestion = ingest_full_text(
        FullTextSource(paper_id="2605.14259v1", source_type="markdown", source_path=source_path)
    )
    expected_quality = assess_full_text_quality(source_path.read_text(encoding="utf-8"))

    result = load_article_source(source_path, log_path=tmp_path / "low-quality.jsonl")

    _assert_common_metadata(result, source_path, "markdown", "text/markdown")
    assert expected_ingestion.extraction_mode == "low_quality_source"
    assert expected_quality.status == "no_substantive_body"
    assert result.outcome == "failed"
    assert result.failure_reason == expected_quality.fallback_reason == expected_ingestion.fallback_reason
    assert result.quality.status == expected_ingestion.quality.status == expected_quality.status
    assert result.quality.heading_count == expected_ingestion.quality.heading_count == expected_quality.heading_count
    assert (
        result.quality.non_heading_nonempty_line_count
        == expected_ingestion.quality.non_heading_nonempty_line_count
        == expected_quality.non_heading_nonempty_line_count
    )
    assert result.text is None
    assert result.warning_count == 1
    assert result.warnings == expected_ingestion.warnings == expected_quality.warnings


@pytest.mark.parametrize(
    ("fixture_name", "source_type", "expected_extraction_mode", "expected_fallback_reason"),
    [
        ("structured_paper.md", "markdown", "structured_markdown", None),
        ("ocr_text.txt", "text", "plain_text", "unstructured_text"),
        ("arxiv_landing_only.md", "markdown", "low_quality_source", "no_substantive_body"),
    ],
)
def test_loader_text_quality_matches_full_text_ingestion_contract(
    tmp_path: Path,
    fixture_name: str,
    source_type: str,
    expected_extraction_mode: str,
    expected_fallback_reason: str | None,
) -> None:
    source_path = FIXTURES_DIR / fixture_name
    ingestion = ingest_full_text(FullTextSource("2605.compat", source_type, source_path))

    result = load_article_source(
        ArticleLoadSource(source_path, paper_id="2605.compat", source_type=source_type),
        log_path=tmp_path / f"{fixture_name}.jsonl",
    )

    assert ingestion.extraction_mode == expected_extraction_mode
    assert ingestion.fallback_reason == expected_fallback_reason
    assert result.quality is not None
    assert result.quality.status == ingestion.quality.status
    assert result.quality.heading_count == ingestion.quality.heading_count
    assert result.quality.non_heading_nonempty_line_count == ingestion.quality.non_heading_nonempty_line_count
    assert result.quality.fallback_reason == ingestion.quality.fallback_reason
    assert result.warnings == ([] if result.outcome == "loaded" else ingestion.warnings)
    if ingestion.quality.status == "no_substantive_body":
        assert result.outcome == "failed"
        assert result.failure_reason == ingestion.fallback_reason
        assert result.text is None
    else:
        assert result.outcome == "loaded"
        assert result.failure_reason is None
        assert result.text == ingestion.text


def test_source_ids_are_stable_across_repeated_loads_and_log_paths(tmp_path: Path) -> None:
    source_path = FIXTURES_DIR / "structured_paper.md"

    first = load_article_source(source_path, log_path=tmp_path / "first" / "article-loader.jsonl")
    second = load_article_source(source_path, log_path=tmp_path / "second" / "article-loader.jsonl")
    classified = classify_article_source(source_path)

    assert first.source_id == second.source_id == classified.source_id
    assert first.sha256 == second.sha256 == classified.sha256
    assert first.provenance["source_id"] == first.source_id
    assert second.provenance["source_id"] == second.source_id


def test_each_load_attempt_emits_exactly_one_terminal_event(tmp_path: Path) -> None:
    log_path = tmp_path / "attempts.jsonl"

    successful = load_article_source(FIXTURES_DIR / "structured_paper.md", log_path=log_path)
    failed = load_article_source(tmp_path / "missing.md", log_path=log_path)

    events = _read_events(log_path)
    by_source_id = {successful.source_id: [], failed.source_id: []}
    for event in events:
        by_source_id[event["source_id"]].append(event["event"])

    assert by_source_id[successful.source_id] == ["source.load_started", "source.load_completed"]
    assert by_source_id[failed.source_id] == ["source.load_started", "source.load_failed"]
    terminal_events = [event for event in events if event["event"] in {"source.load_completed", "source.load_failed"}]
    assert len(terminal_events) == 2
    assert all(event["outcome"] in {"loaded", "loaded_metadata_only", "failed"} for event in terminal_events)
    _assert_safe_event_payload(events)


def test_event_payloads_align_with_source_reference_provenance_fields(tmp_path: Path) -> None:
    source_path = FIXTURES_DIR / "structured_paper.md"
    log_path = tmp_path / "source-reference.jsonl"

    result = load_article_source(
        ArticleLoadSource(source_path, paper_id="2605.12345", source_type="markdown"),
        log_path=log_path,
    )

    events = _read_events(log_path)
    required_keys = {
        "source_id",
        "paper_id",
        "source_path",
        "sha256",
        "media_type",
        "source_type",
        "parser_name",
        "loader_name",
        "outcome",
    }
    for event in events:
        assert required_keys <= event.keys()
        assert event["paper_id"] == "2605.12345"
        assert event["source_id"] == result.source_id
        assert event["source_path"] == str(source_path)
        assert event["sha256"] == result.sha256
        assert event["media_type"] == result.media_type
        assert event["outcome"] in {"started", "loaded"}

    for key in required_keys - {"outcome"}:
        assert key in result.provenance
    assert result.provenance["paper_id"] == "2605.12345"
    assert result.outcome == "loaded"
    _assert_safe_event_payload(events)


def test_secret_like_source_text_is_available_to_result_but_redacted_from_logs(tmp_path: Path) -> None:
    source_path = tmp_path / "secret-bearing.md"
    source_path.write_text(
        "# Local fixture\n\n"
        "This source mentions OPENAI_API_KEY=sk-test-secret1234567890 inside article text.\n"
        "The loader may return source text but must never place that text in JSONL logs.\n",
        encoding="utf-8",
    )
    log_path = tmp_path / "secret-bearing.jsonl"

    result = load_article_source(source_path, log_path=log_path)

    assert result.outcome == "loaded"
    assert result.text is not None
    assert "OPENAI_API_KEY=sk-test-secret" in result.text
    _assert_safe_event_payload(_read_events(log_path))


def test_logging_contract_for_failures_contains_metadata_not_payloads(tmp_path: Path) -> None:
    source_path = tmp_path / "binary.html"
    source_path.write_bytes(b"\xff\xfe\x00<html>not utf-8</html>\x80")
    log_path = tmp_path / "failure.jsonl"

    result = load_article_source(source_path, log_path=log_path)

    assert result.outcome == "failed"
    assert result.failure_reason == "decode_failed"
    events = _read_events(log_path)
    assert [event["event"] for event in events] == ["source.load_started", "source.load_failed"]
    failed = events[-1]
    assert failed["phase"] == "article_loader"
    assert failed["source_path"] == str(source_path)
    assert failed["source_id"] == result.source_id
    assert failed["source_type"] == "html"
    assert failed["media_type"] == "text/html"
    assert failed["sha256"] == result.sha256
    assert failed["byte_size"] == result.byte_size
    assert failed["parser_name"] == "html_loader"
    assert failed["outcome"] == "failed"
    assert failed["failure_reason"] == "decode_failed"
    assert failed["warning_count"] == 1
    _assert_safe_event_payload(events)
