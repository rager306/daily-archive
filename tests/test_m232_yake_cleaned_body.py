"""M232 S01: YAKE inject uses cleaned body, aligned with preprocess spans."""

from __future__ import annotations

import json
from pathlib import Path
from unittest.mock import patch

from research_graph.application.corpus.body_text_clean import clean_body_text
from research_graph.workflows.composition.hybrid_readiness_handoff import (
    HybridReadinessHandoffRequest,
    run_hybrid_readiness_handoff,
)
from research_graph.workflows.composition.yake_keyword_inject import (
    cleaned_body_for_yake,
)


def test_cleaned_body_for_yake_collapses_noise() -> None:
    raw = "Graph   neural\u00a0networks.\n\n\nResults."
    cleaned = cleaned_body_for_yake(raw, is_html=False)
    assert cleaned == clean_body_text(raw).text
    assert "\u00a0" not in cleaned
    assert "\n\n\n" not in cleaned


def test_cleaned_body_for_yake_html_strips_nav_boilerplate() -> None:
    html = """
    <html><body>
    <nav>Home About Careers Subscribe Cookie Policy</nav>
    <article>
      <h1>PageIndex Retrieval</h1>
      <p>Vector search and hierarchical indexing improve recall for long papers.</p>
    </article>
    <footer>Copyright 2025</footer>
    </body></html>
    """
    cleaned = cleaned_body_for_yake(html, is_html=True)
    assert "PageIndex" in cleaned or "vector" in cleaned.casefold()
    # nav boilerplate should be reduced vs raw HTML dump
    assert cleaned.count("Subscribe") <= html.count("Subscribe")


def _handoff(tmp_path: Path, body: str) -> HybridReadinessHandoffRequest:
    sel = {
        "schema_version": "hybrid-gate-selection.v1",
        "milestone_id": "M232-fixture",
        "count": 1,
        "papers": [{"paper_id": "ok1", "category": "cs-cl", "pdf_path": "a.pdf"}],
    }
    index = {
        "articles": [
            {
                "article_ref": "arxiv/cs-cl/ok1",
                "source_code": "arxiv",
                "article_path": "article_catalog/arxiv/cs-cl/ok1/article.json",
            }
        ]
    }
    art = tmp_path / "article_catalog" / "arxiv" / "cs-cl" / "ok1" / "article.json"
    art.parent.mkdir(parents=True)
    art.write_text("{}", encoding="utf-8")
    body_path = tmp_path / "bodies" / "ok1" / "body" / "ok1.hybrid.body.md"
    body_path.parent.mkdir(parents=True)
    body_path.write_text(body, encoding="utf-8")
    sel_path = tmp_path / "sel.json"
    idx_path = tmp_path / "index.json"
    sel_path.write_text(json.dumps(sel), encoding="utf-8")
    idx_path.write_text(json.dumps(index), encoding="utf-8")
    return HybridReadinessHandoffRequest(
        hybrid_selection_path=sel_path,
        body_root=tmp_path / "bodies",
        catalog_index_path=idx_path,
        catalog_root=tmp_path,
        repo_root=tmp_path,
        review_completed=True,
        use_yake_keywords=True,
    )


def test_hybrid_yake_receives_cleaned_text(tmp_path: Path) -> None:
    raw = (
        "Graph   neural\u00a0networks process graph-structured data.\n\n\n"
        "Citation graphs and molecular graphs are common application domains.\n"
        "Enough scholarly prose for YAKE keywords and structure readiness.\n"
    )
    expected = cleaned_body_for_yake(raw, is_html=False)
    with patch(
        "research_graph.workflows.composition.hybrid_readiness_handoff.yake_keywords_for_text",
        return_value=["graph neural", "citation graphs"],
    ) as mock_yake:
        result = run_hybrid_readiness_handoff(_handoff(tmp_path, raw))
    assert mock_yake.called
    yake_arg = mock_yake.call_args.args[0]
    assert yake_arg == expected
    assert "\u00a0" not in yake_arg
    row = result.preprocess_bodies[0]
    assert row["keyword_source"] == "injected"
    assert row["yake_input_chars"] == len(expected)
    assert result.import_eligible is False


def test_hybrid_default_still_token_frequency(tmp_path: Path) -> None:
    raw = "Graph neural networks process graph-structured data. " * 5
    sel = _handoff(tmp_path, raw)
    req = HybridReadinessHandoffRequest(
        hybrid_selection_path=sel.hybrid_selection_path,
        body_root=sel.body_root,
        catalog_index_path=sel.catalog_index_path,
        catalog_root=sel.catalog_root,
        repo_root=sel.repo_root,
        review_completed=True,
        use_yake_keywords=False,
    )
    result = run_hybrid_readiness_handoff(req)
    assert result.preprocess_bodies[0]["keyword_source"] == "token_frequency"
    assert "yake_input_chars" not in result.preprocess_bodies[0]
