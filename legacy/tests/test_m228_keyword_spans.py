"""M228 S01: keyword span locator."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.keyword_spans import (
    KeywordSpan,
    KeywordSpanResult,
    locate_keyword_spans,
)


def test_empty_text_or_keywords() -> None:
    r = locate_keyword_spans("", ["graph"])
    assert r.spans == ()
    assert r.import_eligible is False
    r2 = locate_keyword_spans("graph neural network", [])
    assert r2.spans == ()


def test_casefold_match_with_offsets() -> None:
    text = "Graph neural networks. The GRAPH model works."
    result = locate_keyword_spans(text, ["graph"])
    assert len(result.spans) >= 2
    first = result.spans[0]
    assert first.keyword == "graph"
    assert text[first.start : first.end].casefold() == "graph"
    assert first.start < first.end


def test_max_per_keyword_cap() -> None:
    text = " ".join(["alpha"] * 20)
    result = locate_keyword_spans(text, ["alpha"], max_per_keyword=3)
    assert len(result.spans) == 3


def test_ordered_by_start() -> None:
    text = "end start middle start again"
    result = locate_keyword_spans(text, ["start", "end", "middle"])
    starts = [s.start for s in result.spans]
    assert starts == sorted(starts)


def test_rejects_import_true() -> None:
    with pytest.raises(ValueError, match="import"):
        KeywordSpanResult(spans=(), import_eligible=True)


def test_span_rejects_import_true() -> None:
    with pytest.raises(ValueError, match="import"):
        KeywordSpan(keyword="x", start=0, end=1, import_eligible=True)
