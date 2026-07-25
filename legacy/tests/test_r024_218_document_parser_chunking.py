"""Tests for M121 S04: parser+chunking on the 221-article catalog."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

REPO_ROOT = Path("/root/daily-archive")
INDEX = REPO_ROOT / "data" / "article_catalog" / "article_catalog" / "index.json"
OUTPUT_DIR = REPO_ROOT / "data" / "r024-218-document-corpus-v1" / "parser-chunking"
EVENTS_LOG = OUTPUT_DIR / "events.jsonl"
SUMMARY = OUTPUT_DIR / "summary.json"
CACHE_DIR = REPO_ROOT / "data" / "r024-218-document-corpus-v1" / "pdf-text-cache"
EXPECTED_TOTAL = 221
EXPECTED_COMPLETED = 219
EXPECTED_SKIPPED = 2


def _load(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text())
    return data if isinstance(data, dict) else {}


def _events() -> list[dict[str, Any]]:
    return [json.loads(line) for line in EVENTS_LOG.read_text().splitlines() if line.strip()]


def _index_articles() -> list[dict[str, Any]]:
    data = _load(INDEX)
    articles = data.get("articles", [])
    return [dict(item) for item in articles]


def test_output_dir_exists() -> None:
    assert OUTPUT_DIR.exists()


def test_pdf_text_cache_exists_and_has_many_entries() -> None:
    assert CACHE_DIR.exists()
    cached = list(CACHE_DIR.glob("*.txt"))
    assert len(cached) >= 198


def test_events_log_exists() -> None:
    assert EVENTS_LOG.exists()


def test_summary_exists() -> None:
    assert SUMMARY.exists()


def test_summary_fail_closed() -> None:
    summary = _load(SUMMARY)
    assert summary["network_fetch_attempted"] is False
    assert summary["production_import_attempted"] is False
    assert summary["graph_import_allowed"] is False
    assert summary["ladybugdb_written"] is False


def test_summary_source_backed_records_complete_and_metadata_only_records_skip() -> None:
    summary = _load(SUMMARY)
    assert int(summary["total"]) == EXPECTED_TOTAL
    assert int(summary["ok"]) == EXPECTED_COMPLETED
    assert int(summary["skipped"]) == EXPECTED_SKIPPED
    assert int(summary["errors"]) == 0
    assert summary["skip_reason_counts"] == {"metadata_only_no_local_source_artifact": 2}


def test_summary_chunk_counts_positive() -> None:
    summary = _load(SUMMARY)
    assert int(summary["chunk_count_min"]) > 0
    assert int(summary["chunk_count_total"]) >= EXPECTED_COMPLETED
    assert int(summary["chunk_count_max"]) >= int(summary["chunk_count_min"])


def test_source_kinds_present() -> None:
    summary = _load(SUMMARY)
    counts = summary["source_kind_counts"]
    assert int(counts.get("pdf_converted", 0)) >= 198
    assert int(counts.get("html_native", 0)) >= 1


def test_events_have_fail_closed_flags() -> None:
    ok = [event for event in _events() if event.get("event") == "parser_chunking_complete"]
    skipped = [
        event
        for event in _events()
        if event.get("event") == "parser_chunking_skipped_metadata_only"
    ]
    assert len(ok) == EXPECTED_COMPLETED
    assert len(skipped) == EXPECTED_SKIPPED
    for event in skipped:
        assert event["network_fetch_attempted"] is False
        assert event["production_import_attempted"] is False
        assert event["graph_import_allowed"] is False
        assert event["ladybugdb_written"] is False
        assert event["skip_reason"] == "metadata_only_no_local_source_artifact"
    for event in ok:
        assert event["network_fetch_attempted"] is False
        assert event["production_import_attempted"] is False
        assert event["graph_import_allowed"] is False
        assert event["ladybugdb_written"] is False


def test_events_have_positive_chunk_counts_and_sources() -> None:
    for event in _events():
        if event.get("event") != "parser_chunking_complete":
            continue
        assert int(event["chunk_count"]) > 0
        assert int(event["text_chars"]) > 0
        source = REPO_ROOT / str(event["text_source"])
        assert source.exists()


def test_index_matches_complete_and_skip_events() -> None:
    index_refs = {str(article["article_ref"]) for article in _index_articles()}
    event_refs = {str(event["article_ref"]) for event in _events()}
    assert len(index_refs) == EXPECTED_TOTAL
    assert len(event_refs) == EXPECTED_TOTAL
    assert index_refs == event_refs


def test_no_error_events() -> None:
    errors = [event for event in _events() if event.get("event") == "parser_chunking_error"]
    assert errors == []


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
