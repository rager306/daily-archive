"""M225 S02: markdown outline signals helper."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.outline_signals import (
    OutlineSignals,
    extract_outline_signals,
)


def test_empty_text_empty_signals() -> None:
    signals = extract_outline_signals("")
    assert signals.headings == ()
    assert signals.import_eligible is False


def test_atx_markdown_headings() -> None:
    text = "# Title\n\nIntro.\n\n## Methods\n\nBody.\n\n### Details\n\nMore.\n"
    signals = extract_outline_signals(text)
    titles = [h.text for h in signals.headings]
    assert titles == ["Title", "Methods", "Details"]
    assert signals.headings[0].level == 1
    assert signals.headings[1].level == 2
    assert signals.headings[2].level == 3


def test_numbered_heading_candidates() -> None:
    text = "1. Introduction\nSome prose.\n1.1 Background\nMore.\n2 Results\n"
    signals = extract_outline_signals(text)
    texts = [h.text for h in signals.headings]
    assert any("Introduction" in t for t in texts)
    assert any("Background" in t for t in texts)
    assert any("Results" in t for t in texts)


def test_mixed_atx_preferred_order() -> None:
    text = "# Root\n1.1 Nested\n## Section\n"
    signals = extract_outline_signals(text)
    assert signals.headings[0].text == "Root"
    assert signals.headings[0].source == "atx"
    assert any(h.source == "numbered" for h in signals.headings)


def test_outline_rejects_import_true() -> None:
    with pytest.raises(ValueError, match="import"):
        OutlineSignals(headings=(), import_eligible=True)
