"""Smoke and property tests for the dedicated ingestion loader stack."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st

from research_graph.infrastructure.corpus.ingestion.loader import (
    ArticleLoadSource,
    classify_article_source,
    load_article_source,
)

FIXTURES_DIR = Path(__file__).parent / "fixtures" / "ingestion"
FULL_TEXT_FIXTURES_DIR = Path(__file__).parent / "fixtures" / "full_text"

TEXT_OR_BINARY_CASES = [
    ("structured_paper.md", "markdown", "text/markdown", "markdown_loader", "loaded"),
    ("minimal_article.html", "html", "text/html", "html_loader", "loaded"),
    ("ocr_text.txt", "text", "text/plain", "text_loader", "loaded"),
    ("minimal.pdf", "pdf", "application/pdf", "pdf_metadata_probe", "loaded_metadata_only"),
]

REQUIRED_EVENT_KEYS = {
    "ts",
    "event",
    "phase",
    "status",
    "source_path",
    "source_id",
    "source_type",
    "media_type",
    "sha256",
    "checksum",
    "byte_size",
    "parser_name",
    "loader_name",
    "outcome",
    "selected_fallback",
    "failure_reason",
    "duration_ms",
    "warning_count",
}

FORBIDDEN_LOG_SNIPPETS = [
    "%PDF-1.4",
    "Graph-Guided Retrieval for Scientific Agents",
    "This HTML article fixture exercises deterministic local source classification.",
    "OCR PAGE 1",
    "OPENAI_API_KEY",
    "raw_text",
    "raw_bytes",
    "binary_payload",
    "embedding",
    "vector",
]

SOURCE_SUFFIXES = st.sampled_from([".md", ".html", ".htm", ".txt", ".pdf", ".xyz"])
SOURCE_BYTES = st.binary(min_size=0, max_size=96)


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _read_events(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def _assert_metadata_matches_source(record: object, source_path: Path) -> None:
    assert record.source_path == source_path
    assert record.sha256 == _sha256(source_path)
    assert record.byte_size == source_path.stat().st_size
    assert record.source_id.startswith("article-source:")


def _assert_event_shape(event: dict[str, Any], result: object, source_path: Path) -> None:
    assert REQUIRED_EVENT_KEYS <= event.keys()
    assert event["phase"] == "article_loader"
    assert event["source_path"] == str(source_path)
    assert event["source_id"] == result.source_id
    assert event["source_type"] == result.source_type
    assert event["media_type"] == result.media_type
    assert event["sha256"] == result.sha256
    assert event["checksum"] == result.sha256
    assert event["byte_size"] == result.byte_size
    assert event["parser_name"] == result.parser_name
    assert event["loader_name"] == "local_article_loader"
    assert event["duration_ms"] >= 0
    assert isinstance(event["warning_count"], int)


def _assert_logs_are_metadata_only(events: list[dict[str, Any]]) -> None:
    serialized = json.dumps(events, sort_keys=True)
    for forbidden in FORBIDDEN_LOG_SNIPPETS:
        assert forbidden not in serialized


@pytest.mark.parametrize(
    ("fixture_name", "source_type", "media_type", "parser_name", "outcome"),
    TEXT_OR_BINARY_CASES,
)
def test_fixture_sources_smoke_load_with_structured_provenance_events(
    tmp_path: Path,
    fixture_name: str,
    source_type: str,
    media_type: str,
    parser_name: str,
    outcome: str,
) -> None:
    source_path = FIXTURES_DIR / fixture_name
    log_path = tmp_path / f"{fixture_name}.jsonl"

    classified = classify_article_source(source_path)
    result = load_article_source(
        ArticleLoadSource(source_path, paper_id="2605.fixture", source_type="auto"),
        log_path=log_path,
    )

    _assert_metadata_matches_source(classified, source_path)
    _assert_metadata_matches_source(result, source_path)
    assert classified.source_type == result.source_type == source_type
    assert classified.media_type == result.media_type == media_type
    assert classified.parser_name == result.parser_name == parser_name
    assert classified.source_id == result.source_id
    assert result.loader_name == "local_article_loader"
    assert result.outcome == outcome
    assert result.failure_reason is None
    assert result.provenance is not None
    assert result.provenance["source_id"] == result.source_id
    assert result.provenance["source_path"] == str(source_path)
    assert result.provenance["source_type"] == source_type
    assert result.provenance["sha256"] == result.sha256
    assert result.provenance["parser_name"] == parser_name
    assert result.provenance["loader_name"] == "local_article_loader"
    assert result.provenance["paper_id"] == "2605.fixture"

    events = _read_events(log_path)
    assert [event["event"] for event in events] == ["source.load_started", "source.load_completed"]
    assert events[0]["outcome"] == "started"
    assert events[-1]["outcome"] == outcome
    assert events[-1]["selected_fallback"] is None
    assert events[-1]["failure_reason"] is None
    for event in events:
        _assert_event_shape(event, result, source_path)
        assert event["paper_id"] == "2605.fixture"
    _assert_logs_are_metadata_only(events)


def test_real_arxiv_landing_smoke_fails_with_low_quality_fallback_and_failure_log(
    tmp_path: Path,
) -> None:
    source_path = FULL_TEXT_FIXTURES_DIR / "arxiv_landing_only.md"
    log_path = tmp_path / "arxiv-landing.jsonl"

    result = load_article_source(
        ArticleLoadSource(source_path, paper_id="2605.14259v1", source_type="markdown"),
        log_path=log_path,
    )

    _assert_metadata_matches_source(result, source_path)
    assert result.source_type == "markdown"
    assert result.media_type == "text/markdown"
    assert result.parser_name == "markdown_loader"
    assert result.outcome == "failed"
    assert result.failure_reason == "no_substantive_body"
    assert result.text is None
    assert result.quality is not None
    assert result.quality.status == "no_substantive_body"
    assert result.quality.fallback_reason == "no_substantive_body"
    assert result.warning_count == 1

    events = _read_events(log_path)
    assert [event["event"] for event in events] == ["source.load_started", "source.load_failed"]
    failed_event = events[-1]
    _assert_event_shape(failed_event, result, source_path)
    assert failed_event["paper_id"] == "2605.14259v1"
    assert failed_event["status"] == "failed"
    assert failed_event["selected_fallback"] == "no_substantive_body"
    assert failed_event["failure_reason"] == "no_substantive_body"
    assert failed_event["warning_count"] == 1
    _assert_logs_are_metadata_only(events)


@pytest.mark.parametrize(
    ("fixture_name", "failure_reason", "expected_type", "expected_parser"),
    [
        ("unsupported.bin", "unsupported_type", "unsupported", "unsupported_loader"),
        ("binary.md", "decode_failed", "markdown", "markdown_loader"),
        ("empty.txt", "source_empty", "text", "text_loader"),
    ],
)
def test_loader_failure_classification_is_typed_and_logged(
    tmp_path: Path,
    fixture_name: str,
    failure_reason: str,
    expected_type: str,
    expected_parser: str,
) -> None:
    source_path = tmp_path / fixture_name
    if failure_reason == "unsupported_type":
        source_path.write_text("substantive text under an unsupported extension", encoding="utf-8")
    elif failure_reason == "decode_failed":
        source_path.write_bytes(b"\xff\xfe\x00not utf-8\x80")
    else:
        source_path.write_text("  \n\t\n", encoding="utf-8")

    result = load_article_source(source_path, log_path=tmp_path / f"{fixture_name}.jsonl")

    _assert_metadata_matches_source(result, source_path)
    assert result.outcome == "failed"
    assert result.failure_reason == failure_reason
    assert result.source_type == expected_type
    assert result.parser_name == expected_parser
    assert result.text is None
    assert result.warning_count == 1

    failed_event = _read_events(tmp_path / f"{fixture_name}.jsonl")[-1]
    _assert_event_shape(failed_event, result, source_path)
    assert failed_event["event"] == "source.load_failed"
    assert failed_event["selected_fallback"] == failure_reason
    assert failed_event["failure_reason"] == failure_reason


def test_missing_source_failure_keeps_path_and_reason_without_checksum(tmp_path: Path) -> None:
    source_path = tmp_path / "missing.md"
    log_path = tmp_path / "missing.jsonl"

    result = load_article_source(source_path, log_path=log_path)

    assert result.source_path == source_path
    assert result.source_type == "unknown"
    assert result.media_type == "application/octet-stream"
    assert result.sha256 is None
    assert result.byte_size == 0
    assert result.outcome == "failed"
    assert result.failure_reason == "source_missing"
    assert result.provenance is not None
    assert result.provenance["source_path"] == str(source_path)
    assert result.provenance["source_type"] == "unknown"
    assert result.provenance["sha256"] is None

    failed_event = _read_events(log_path)[-1]
    _assert_event_shape(failed_event, result, source_path)
    assert failed_event["event"] == "source.load_failed"
    assert failed_event["checksum"] is None
    assert failed_event["selected_fallback"] == "source_missing"
    assert failed_event["failure_reason"] == "source_missing"


@settings(max_examples=30, deadline=None)
@given(suffix=SOURCE_SUFFIXES, payload=SOURCE_BYTES)
def test_source_identity_and_terminal_event_shape_are_stable_for_generated_local_files(
    suffix: str,
    payload: bytes,
) -> None:
    with TemporaryDirectory() as temp_dir:
        tmp_path = Path(temp_dir)
        source_path = tmp_path / f"generated{suffix}"
        source_path.write_bytes(payload)
        first_log = tmp_path / "first.jsonl"
        second_log = tmp_path / "second.jsonl"

        first = load_article_source(source_path, log_path=first_log)
        second = load_article_source(source_path, log_path=second_log)

        assert first.source_id == second.source_id
        assert first.sha256 == second.sha256 == _sha256(source_path)
        assert first.byte_size == second.byte_size == len(payload)
        assert first.source_path == second.source_path == source_path
        assert first.source_type == second.source_type
        assert first.media_type == second.media_type
        assert first.parser_name == second.parser_name
        assert first.outcome in {"loaded", "loaded_metadata_only", "failed"}

        first_events = _read_events(first_log)
        second_events = _read_events(second_log)
        assert [event["event"] for event in first_events][0] == "source.load_started"
        assert (
            len(
                [
                    event
                    for event in first_events
                    if event["event"] in {"source.load_completed", "source.load_failed"}
                ]
            )
            == 1
        )
        assert first_events[-1]["event"] in {"source.load_completed", "source.load_failed"}
        assert second_events[-1]["source_id"] == first_events[-1]["source_id"] == first.source_id
        for event in first_events + second_events:
            _assert_event_shape(event, first if event in first_events else second, source_path)
            assert event["selected_fallback"] == event["failure_reason"]
        _assert_logs_are_metadata_only(first_events + second_events)
