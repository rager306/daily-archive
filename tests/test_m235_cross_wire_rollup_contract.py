"""M235 S02: hybrid and non_arxiv share preprocess_rollup contract."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.application.corpus.preprocess_rollup import empty_preprocess_rollup
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

_REQUIRED = frozenset(
    {
        "body_count",
        "quality_status_counts",
        "keyword_source_counts",
        "drives_verdict",
        "import_eligible",
    }
)

_BODY = """# Graph Neural Networks

## Abstract
Graph neural networks process graph-structured data using message passing.

## Method
We evaluate citation graphs and molecular graphs for prediction tasks.

## Results
Enough scholarly prose for structure readiness and preprocess rollup checks.
"""


def _assert_rollup_contract(rollup: dict) -> None:
    assert _REQUIRED <= set(rollup.keys())
    assert rollup["drives_verdict"] is False
    assert rollup["import_eligible"] is False
    assert isinstance(rollup["body_count"], int)
    assert isinstance(rollup["quality_status_counts"], dict)
    assert isinstance(rollup["keyword_source_counts"], dict)


def test_empty_factory_matches_contract() -> None:
    _assert_rollup_contract(empty_preprocess_rollup())


def test_hybrid_rollup_contract(tmp_path: Path) -> None:
    sel = {
        "schema_version": "m213-hybrid-gate-selection.v1",
        "milestone_id": "M235-contract",
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
    body.write_text(_BODY, encoding="utf-8")
    sel_path = tmp_path / "sel.json"
    idx_path = tmp_path / "index.json"
    sel_path.write_text(json.dumps(sel), encoding="utf-8")
    idx_path.write_text(json.dumps(index), encoding="utf-8")
    result = run_hybrid_readiness_handoff(
        HybridReadinessHandoffRequest(
            hybrid_selection_path=sel_path,
            body_root=tmp_path / "bodies",
            catalog_index_path=idx_path,
            catalog_root=tmp_path,
            repo_root=tmp_path,
            review_completed=True,
        )
    )
    assert result.import_eligible is False
    _assert_rollup_contract(result.preprocess_rollup)
    assert result.preprocess_rollup["body_count"] == 1
    _assert_rollup_contract(result.to_dict()["preprocess_rollup"])


def test_non_arxiv_rollup_contract() -> None:
    if not BLOG_ARTICLE.is_file():
        return
    result = run_non_arxiv_html_source_proof(
        NonArxivHtmlSourceProofRequest(
            article_json_path=BLOG_ARTICLE,
            repo_root=ROOT,
        )
    )
    assert result.import_eligible is False
    _assert_rollup_contract(result.preprocess_rollup)
    assert result.preprocess_rollup["body_count"] == 1
    _assert_rollup_contract(result.to_dict()["preprocess_rollup"])
