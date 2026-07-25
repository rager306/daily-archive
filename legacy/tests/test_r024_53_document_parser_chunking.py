"""Tests for R024 S02: parser+chunking on 53 articles (PDF + HTML)."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

REPO_ROOT = Path("/root/daily-archive")
SELECTION = REPO_ROOT / "data" / "r024-53-document-corpus-v1" / "selection.json"
OUTPUT_DIR = REPO_ROOT / "data" / "r024-53-document-corpus-v1" / "parser-chunking"
EVENTS_LOG = OUTPUT_DIR / "events.jsonl"
SUMMARY = OUTPUT_DIR / "summary.json"
CACHE_DIR = REPO_ROOT / "data" / "r024-53-document-corpus-v1" / "pdf-text-cache"


def _load(path: Path) -> dict[str, object]:
    data = json.loads(path.read_text())
    return dict(data) if isinstance(data, dict) else {}


def _load_articles(path: Path) -> list[dict[str, object]]:
    data = json.loads(path.read_text())
    if isinstance(data, dict) and isinstance(data.get("articles"), list):
        return [dict(item) for item in data["articles"]]
    return []


def test_output_dir_exists() -> None:
    assert OUTPUT_DIR.exists()


def test_pdf_cache_exists() -> None:
    assert CACHE_DIR.exists()
    cached = list(CACHE_DIR.glob("*.txt"))
    assert len(cached) >= 30


def test_events_log_exists() -> None:
    assert EVENTS_LOG.exists()


def test_summary_exists() -> None:
    assert SUMMARY.exists()


def test_summary_fail_closed() -> None:
    s = _load(SUMMARY)
    assert s["network_fetch_attempted"] is False
    assert s["production_import_attempted"] is False
    assert s["graph_import_allowed"] is False
    assert s["ladybugdb_written"] is False


def test_summary_53_ok() -> None:
    s = _load(SUMMARY)
    assert int(str(s.get("total", 0))) == 53
    assert int(str(s.get("ok", 0))) == 53
    assert int(str(s.get("errors", 0))) == 0


def test_events_have_fail_closed() -> None:
    events = [
        dict(json.loads(line)) for line in EVENTS_LOG.read_text().splitlines() if line.strip()
    ]
    ok = [e for e in events if e.get("event") == "parser_chunking_complete"]
    assert len(ok) == 53
    for e in ok:
        assert e["network_fetch_attempted"] is False
        assert e["production_import_attempted"] is False
        assert e["graph_import_allowed"] is False
        assert e["ladybugdb_written"] is False


def test_chunk_count_positive() -> None:
    events = [
        dict(json.loads(line)) for line in EVENTS_LOG.read_text().splitlines() if line.strip()
    ]
    for e in events:
        if e.get("event") == "parser_chunking_complete":
            assert int(str(e["chunk_count"])) > 0


def test_source_kinds_present() -> None:
    """PDF and HTML both processed."""
    events = [
        dict(json.loads(line)) for line in EVENTS_LOG.read_text().splitlines() if line.strip()
    ]
    ok = [e for e in events if e.get("event") == "parser_chunking_complete"]
    pdf_count = sum(1 for e in ok if e.get("source_kind") == "pdf_converted")
    html_count = sum(1 for e in ok if e.get("source_kind") == "html_native")
    assert pdf_count >= 30
    assert html_count >= 1


def test_selection_matches_events() -> None:
    articles = _load_articles(SELECTION)
    sel_refs = {str(a["article_ref"]) for a in articles}
    events = [
        dict(json.loads(line)) for line in EVENTS_LOG.read_text().splitlines() if line.strip()
    ]
    event_refs = {str(e["article_ref"]) for e in events}
    assert sel_refs == event_refs


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
