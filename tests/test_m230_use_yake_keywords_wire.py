"""M230 S03: optional use_yake_keywords on hybrid handoff and non_arxiv proof."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.workflows.composition.hybrid_readiness_handoff import (
    HybridReadinessHandoffRequest,
    run_hybrid_readiness_handoff,
)
from research_graph.workflows.composition.non_arxiv_html_source_proof import (
    NonArxivHtmlSourceProofRequest,
    run_non_arxiv_html_source_proof,
)

ROOT = Path(__file__).resolve().parents[1]
BLOG_ARTICLE = (
    ROOT
    / "data/article_catalog/article_catalog/company_blog/cs-ir/"
    / "pageindex_zhang2025pageindex/article.json"
)


def _handoff_fixture(tmp_path: Path) -> HybridReadinessHandoffRequest:
    sel = {
        "schema_version": "m213-hybrid-gate-selection.v1",
        "milestone_id": "M230-fixture",
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
    body = tmp_path / "bodies" / "ok1" / "body" / "ok1.hybrid.body.md"
    body.parent.mkdir(parents=True)
    body.write_text(
        """# Graph Neural Networks

## Abstract
Graph neural networks process graph-structured data using message passing.

## Method
We evaluate citation graphs and molecular graphs for prediction tasks.

## Results
Enough scholarly prose for YAKE keywords and structure readiness.
""",
        encoding="utf-8",
    )
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
    )


def test_hybrid_default_keyword_source_token_frequency(tmp_path: Path) -> None:
    base = _handoff_fixture(tmp_path)
    result = run_hybrid_readiness_handoff(base)
    assert result.import_eligible is False
    assert len(result.preprocess_bodies) == 1
    assert result.preprocess_bodies[0]["keyword_source"] == "token_frequency"
    assert "use_yake_keywords:False" in result.diagnostics


def test_hybrid_use_yake_keywords_injects(tmp_path: Path) -> None:
    base = _handoff_fixture(tmp_path)
    req = HybridReadinessHandoffRequest(
        hybrid_selection_path=base.hybrid_selection_path,
        body_root=base.body_root,
        catalog_index_path=base.catalog_index_path,
        catalog_root=base.catalog_root,
        repo_root=base.repo_root,
        review_completed=True,
        use_yake_keywords=True,
    )
    result = run_hybrid_readiness_handoff(req)
    assert result.import_eligible is False
    assert len(result.preprocess_bodies) == 1
    row = result.preprocess_bodies[0]
    assert row["keyword_source"] == "injected"
    assert isinstance(row["content_keywords"], list)
    assert len(row["content_keywords"]) >= 1
    assert "use_yake_keywords:True" in result.diagnostics


def test_non_arxiv_use_yake_keywords_when_catalog_present() -> None:
    if not BLOG_ARTICLE.is_file():
        return
    off = run_non_arxiv_html_source_proof(
        NonArxivHtmlSourceProofRequest(
            article_json_path=BLOG_ARTICLE,
            repo_root=ROOT,
            use_yake_keywords=False,
        )
    )
    assert off.import_eligible is False
    assert off.preprocess is not None
    assert off.preprocess["keyword_source"] == "token_frequency"

    on = run_non_arxiv_html_source_proof(
        NonArxivHtmlSourceProofRequest(
            article_json_path=BLOG_ARTICLE,
            repo_root=ROOT,
            use_yake_keywords=True,
        )
    )
    assert on.import_eligible is False
    assert on.preprocess is not None
    assert on.preprocess["keyword_source"] == "injected"
    assert len(on.preprocess["content_keywords"]) >= 1
