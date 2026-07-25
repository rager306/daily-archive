"""M227 S01: shared preprocess summary helper."""

from __future__ import annotations

from research_graph.application.corpus.preprocess_summary import (
    preprocess_summary_for_body,
)


def test_scholarly_summary_has_fingerprint_and_language() -> None:
    text = (
        "# Methods\n\n"
        "The method and results of this experiment show that graph neural networks "
        "are effective for citation prediction and molecular property tasks. "
        "Additional scholarly prose for stable heuristics.\n"
    )
    summary = preprocess_summary_for_body(
        source_id="p1",
        text=text,
        source_class="arxiv",
        profile="scholarly",
        is_html=False,
    )
    assert summary["import_eligible"] is False
    assert len(summary["content_fingerprint_sha256"]) == 64
    assert summary["language"] == "en"
    assert summary["word_count"] > 10
    assert "quality_status" in summary
    assert "outline_heading_count" in summary
    assert summary["source_id"] == "p1"


def test_html_web_profile_strips_ops() -> None:
    html = (
        "<html><body><nav>Subscribe</nav><article>"
        "<h1>Title</h1><p>" + ("word " * 80) + "</p></article></body></html>"
    )
    summary = preprocess_summary_for_body(
        source_id="blog",
        text=html,
        source_class="company_blog",
        profile="web",
        is_html=True,
    )
    assert "html_main_content" in summary["clean_ops"]
    assert summary["import_eligible"] is False
