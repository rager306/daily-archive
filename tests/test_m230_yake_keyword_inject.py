"""M230 S02: composition YAKE keyword helper."""

from __future__ import annotations

from research_graph.workflows.composition.yake_keyword_inject import (
    yake_keywords_for_text,
)


def test_empty_text_returns_empty() -> None:
    assert yake_keywords_for_text("") == []
    assert yake_keywords_for_text("   ") == []


def test_yake_returns_keyword_strings() -> None:
    text = (
        "Graph neural networks process graph-structured data using message passing. "
        "Citation graphs and molecular graphs are common application domains for "
        "graph neural network research and evaluation benchmarks."
    )
    kws = yake_keywords_for_text(text, language="en", top_k=8)
    assert isinstance(kws, list)
    assert len(kws) >= 1
    assert all(isinstance(k, str) and k.strip() for k in kws)


def test_application_corpus_still_has_no_yake_import() -> None:
    from pathlib import Path

    corpus = Path(__file__).resolve().parents[1] / "src/research_graph/application/corpus"
    for path in corpus.glob("*.py"):
        src = path.read_text(encoding="utf-8")
        assert "import yake" not in src
        assert "from yake" not in src
