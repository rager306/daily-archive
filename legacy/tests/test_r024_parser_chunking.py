"""Tests for R024 S02: parser+chunking replay on 10-article corpus."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

REPO_ROOT = Path("/root/daily-archive")
SELECTION = REPO_ROOT / "data" / "r024-10-document-corpus-v1" / "selection.json"
OUTPUT_DIR = REPO_ROOT / "data" / "r024-10-document-corpus-v1" / "parser-chunking"
EVENTS_LOG = OUTPUT_DIR / "events.jsonl"
SUMMARY = OUTPUT_DIR / "summary.json"


def _load(path: Path) -> dict[str, object]:
    data = json.loads(path.read_text())
    return dict(data) if isinstance(data, dict) else {}


def _load_articles(path: Path) -> list[dict[str, object]]:
    """Load selection.json (dict with 'articles' key) and return the articles list."""
    data = json.loads(path.read_text())
    if isinstance(data, dict) and isinstance(data.get("articles"), list):
        return [dict(item) for item in data["articles"]]
    return []


def test_output_dir_exists() -> None:
    assert OUTPUT_DIR.exists(), "parser-chunking output dir missing"


def test_events_log_exists() -> None:
    assert EVENTS_LOG.exists(), "events.jsonl missing"


def test_summary_exists() -> None:
    assert SUMMARY.exists(), "summary.json missing"


def test_summary_has_fail_closed_invariants() -> None:
    summary = _load(SUMMARY)
    assert summary["network_fetch_attempted"] is False
    assert summary["production_import_attempted"] is False
    assert summary["graph_import_allowed"] is False
    assert summary["ladybugdb_written"] is False


def test_summary_all_articles_ok() -> None:
    summary = _load(SUMMARY)
    assert summary["total"] == 10
    assert summary["ok"] == 10
    assert summary["errors"] == 0


def test_events_have_fail_closed_invariants() -> None:
    events = [
        dict(json.loads(line)) for line in EVENTS_LOG.read_text().splitlines() if line.strip()
    ]
    assert len(events) == 10
    ok_events = [e for e in events if e.get("event") == "parser_chunking_complete"]
    assert len(ok_events) == 10
    for e in ok_events:
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
            assert e["chunk_count"] > 0, f"{e['article_ref']} chunks=0"


def test_selection_matches_events() -> None:
    """Every selection article has an event."""
    sel_articles = _load_articles(SELECTION)
    events = [
        dict(json.loads(line)) for line in EVENTS_LOG.read_text().splitlines() if line.strip()
    ]
    sel_refs = {str(a["article_ref"]) for a in sel_articles}
    event_refs = {e["article_ref"] for e in events}
    assert sel_refs == event_refs, f"selection≠events: missing={sel_refs - event_refs}"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
