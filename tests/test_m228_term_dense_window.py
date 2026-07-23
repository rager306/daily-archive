"""M228 S02: term-dense evidence window."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.keyword_spans import locate_keyword_spans
from research_graph.application.corpus.term_dense_window import (
    TermDenseWindow,
    term_dense_window,
)


def test_empty_spans_returns_prefix() -> None:
    text = "Hello world this is a body of text for testing windows."
    win = term_dense_window(text, spans=(), max_chars=20)
    assert win.start == 0
    assert win.end <= 20
    assert win.hit_count == 0
    assert win.import_eligible is False
    assert text[win.start : win.end] == win.snippet


def test_prefers_dense_region() -> None:
    # sparse "graph" at start; dense cluster later
    text = (
        "graph alone. filler filler filler filler filler filler. "
        "graph graph model graph neural graph network graph end."
    )
    spans = locate_keyword_spans(text, ["graph"]).spans
    win = term_dense_window(text, spans=spans, max_chars=40)
    assert win.hit_count >= 2
    # densest region is toward the end cluster
    assert win.start > 10


def test_window_bounds_within_text() -> None:
    text = "alpha beta gamma"
    spans = locate_keyword_spans(text, ["beta"]).spans
    win = term_dense_window(text, spans=spans, max_chars=100)
    assert 0 <= win.start < win.end <= len(text)


def test_rejects_import_true() -> None:
    with pytest.raises(ValueError, match="import"):
        TermDenseWindow(
            start=0,
            end=1,
            snippet="x",
            hit_count=0,
            import_eligible=True,
        )
