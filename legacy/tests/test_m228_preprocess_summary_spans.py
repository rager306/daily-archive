"""M228 S03: keyword spans and dense window on preprocess summary."""

from __future__ import annotations

from research_graph.application.corpus.preprocess_summary import (
    preprocess_summary_for_body,
)


def test_summary_includes_keyword_spans_and_window() -> None:
    text = (
        "# Graph Methods\n\n"
        "Graph neural networks pass messages. Graph graph model graph results. "
        "The experiment shows graph effectiveness for citation tasks and more prose "
        "to stabilize language detection and quality scoring for scholarly profile.\n"
    )
    summary = preprocess_summary_for_body(
        source_id="p1",
        text=text,
        source_class="arxiv",
        profile="scholarly",
        is_html=False,
    )
    assert summary["import_eligible"] is False
    assert summary["keyword_span_count"] >= 1
    assert isinstance(summary["content_keywords"], list)
    assert len(summary["content_keywords"]) >= 1
    win = summary["evidence_window"]
    assert win is not None
    assert "snippet" in win
    assert win["hit_count"] >= 0
    assert win["start"] >= 0
    assert win["end"] >= win["start"]


def test_existing_summary_keys_preserved() -> None:
    text = "The method and results show graph neural networks are useful. " * 5
    summary = preprocess_summary_for_body(
        source_id="p2",
        text=text,
        profile="scholarly",
    )
    for key in (
        "language",
        "content_fingerprint_sha256",
        "quality_status",
        "word_count",
        "import_eligible",
    ):
        assert key in summary
