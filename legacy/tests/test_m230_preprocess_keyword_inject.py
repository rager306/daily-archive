"""M230 S01: optional keyword inject into preprocess summary (no YAKE in app)."""

from __future__ import annotations

from research_graph.application.corpus.preprocess_summary import (
    preprocess_summary_for_body,
)


def test_default_keyword_source_is_token_frequency() -> None:
    text = (
        "Graph neural networks pass messages. Graph graph model results. "
        "The experiment shows effectiveness for citation tasks with more prose "
        "to stabilize language and quality scoring for scholarly profile.\n"
    )
    summary = preprocess_summary_for_body(
        source_id="p1",
        text=text,
        profile="scholarly",
    )
    assert summary["keyword_source"] == "token_frequency"
    assert summary["import_eligible"] is False
    assert isinstance(summary["content_keywords"], list)
    assert summary["keyword_span_count"] >= 0


def test_injected_keywords_drive_spans_and_source() -> None:
    text = (
        "Alpha beta gamma. UniqueMarker appears here once. "
        "More scholarly prose about methods and results for scoring.\n"
    )
    summary = preprocess_summary_for_body(
        source_id="p2",
        text=text,
        profile="scholarly",
        keywords=["UniqueMarker", "missingterm"],
    )
    assert summary["keyword_source"] == "injected"
    assert "uniquemarker" in [k.casefold() for k in summary["content_keywords"]]
    assert summary["keyword_span_count"] >= 1
    assert summary["import_eligible"] is False
    # densest window should relate to UniqueMarker region
    assert summary["evidence_window"]["hit_count"] >= 1


def test_empty_injected_list_falls_back_to_token_frequency() -> None:
    text = "Graph neural networks are useful for citation tasks. " * 4
    summary = preprocess_summary_for_body(
        source_id="p3",
        text=text,
        keywords=[],
    )
    assert summary["keyword_source"] == "token_frequency"
