"""Tests for R024 53-document corpus selection (M118 S01)."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

REPO_ROOT = Path("/root/daily-archive")
SELECTION = REPO_ROOT / "data" / "r024-53-document-corpus-v1" / "selection.json"
EVENTS_LOG = REPO_ROOT / "data" / "r024-53-document-corpus-v1" / "selection-events.jsonl"
SUMMARY = REPO_ROOT / "data" / "r024-53-document-corpus-v1" / "selection-summary.json"
CATALOG_ROOT = REPO_ROOT / "data" / "article_catalog" / "article_catalog"


def _load(path: Path) -> dict[str, object]:
    data = json.loads(path.read_text())
    return dict(data) if isinstance(data, dict) else {}


def _load_articles(path: Path) -> list[dict[str, object]]:
    data = json.loads(path.read_text())
    if isinstance(data, dict) and isinstance(data.get("articles"), list):
        return [dict(item) for item in data["articles"]]
    return []


def test_selection_exists() -> None:
    assert SELECTION.exists()


def test_articles_count_53() -> None:
    articles = _load_articles(SELECTION)
    assert len(articles) == 53


def test_unique_keys() -> None:
    articles = _load_articles(SELECTION)
    keys = [str(a["article_key"]) for a in articles]
    assert len(set(keys)) == 53


def test_baseline_20_extension_33() -> None:
    articles = _load_articles(SELECTION)
    baseline = sum(1 for a in articles if "m117" in str(a.get("selection_source", "")))
    extension = sum(1 for a in articles if "m118" in str(a.get("selection_source", "")))
    assert baseline == 20
    assert extension == 33


def test_all_have_extractable_source() -> None:
    articles = _load_articles(SELECTION)
    for a in articles:
        ref = str(a["article_ref"])
        sd = CATALOG_ROOT / ref / "source"
        assert sd.exists(), f"No source dir for {ref}"
        source_kind = str(a.get("source_kind", ""))
        if source_kind == "pdf_converted":
            pdf = sd / f"{a['article_key']}.pdf"
            assert pdf.exists(), f"No PDF for {ref}"
        else:
            has_text = bool(
                list(sd.glob("*.html")) + list(sd.glob("*.md")) + list(sd.glob("*.txt"))
            )
            assert has_text, f"No text source for {ref}"


def test_pdf_conversion_documented() -> None:
    sel = _load(SELECTION)
    assert sel.get("pdf_conversion_library") == "pymupdf 1.27.2.3"
    assert sel.get("pdf_conversion_documented") is True


def test_fail_closed_invariants() -> None:
    sel = _load(SELECTION)
    assert sel.get("network_policy") == "test_phase_must_not_fetch"
    assert sel.get("graph_import_allowed") is False
    assert sel.get("ladybugdb_written") is False
    assert sel.get("production_import_attempted") is False


def test_summary_fail_closed() -> None:
    s = _load(SUMMARY)
    assert s.get("network_fetch_attempted") is False
    assert s.get("graph_import_allowed") is False
    assert s.get("ladybugdb_written") is False


def test_events_log_exists() -> None:
    assert EVENTS_LOG.exists()


def test_selection_matches_events() -> None:
    articles = _load_articles(SELECTION)
    sel_refs = {str(a["article_ref"]) for a in articles}
    event_refs: set[str] = set()
    for line in EVENTS_LOG.read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        ev = json.loads(line)
        if ev.get("event") == "article_selected":
            event_refs.add(str(ev["article_ref"]))
    assert sel_refs == event_refs


def test_events_fail_closed() -> None:
    for line in EVENTS_LOG.read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        ev = json.loads(line)
        if ev.get("event") == "article_selected":
            assert ev["network_fetch_attempted"] is False
            assert ev["graph_import_allowed"] is False
            assert ev["ladybugdb_written"] is False


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
