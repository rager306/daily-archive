"""M231 S02: use_yake_keywords wires map detected language to YAKE lan."""

from __future__ import annotations

import json
from pathlib import Path
from unittest.mock import patch

from research_graph.workflows.composition.hybrid_readiness_handoff import (
    HybridReadinessHandoffRequest,
    run_hybrid_readiness_handoff,
)
from research_graph.workflows.composition.non_arxiv_html_source_proof import (
    NonArxivHtmlSourceProofRequest,
    run_non_arxiv_html_source_proof,
)
from research_graph.workflows.composition.yake_keyword_inject import (
    yake_language_code,
)

ROOT = Path(__file__).resolve().parents[1]
BLOG_ARTICLE = (
    ROOT
    / "data/article_catalog/article_catalog/company_blog/cs-ir/"
    / "pageindex_zhang2025pageindex/article.json"
)


def _handoff_req(tmp_path: Path, body: str, **kwargs) -> HybridReadinessHandoffRequest:
    sel = {
        "schema_version": "hybrid-gate-selection.v1",
        "milestone_id": "M231-fixture",
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
        **kwargs,
    )


_EN_BODY = """# Graph Neural Networks

## Abstract
Graph neural networks process graph-structured data using message passing.

## Method
We evaluate citation graphs and molecular graphs for prediction tasks.

## Results
Enough scholarly prose for YAKE keywords and structure readiness.
"""

_RU_BODY = """
Это исследование графовых нейронных сетей. В работе рассматриваются модели
передачи сообщений на графах цитирования и молекулярных структурах. Результаты
экспериментов показывают эффективность предложенного подхода для задач
классификации вершин и предсказания связей в научных корпусах.
"""


def test_hybrid_default_no_yake_language_field(tmp_path: Path) -> None:
    result = run_hybrid_readiness_handoff(_handoff_req(tmp_path, _EN_BODY))
    assert result.import_eligible is False
    assert result.preprocess_bodies[0]["keyword_source"] == "token_frequency"
    assert "yake_language" not in result.preprocess_bodies[0]
    assert "yake_languages:none" in result.diagnostics


def test_hybrid_en_body_sets_yake_language_en(tmp_path: Path) -> None:
    result = run_hybrid_readiness_handoff(
        _handoff_req(tmp_path, _EN_BODY, use_yake_keywords=True)
    )
    row = result.preprocess_bodies[0]
    assert row["keyword_source"] == "injected"
    assert row["yake_language"] == "en"
    assert "yake_languages:en" in result.diagnostics


def test_hybrid_ru_body_maps_to_yake_ru(tmp_path: Path) -> None:
    result = run_hybrid_readiness_handoff(
        _handoff_req(tmp_path, _RU_BODY, use_yake_keywords=True)
    )
    row = result.preprocess_bodies[0]
    assert row["keyword_source"] == "injected"
    assert row["yake_language"] == "ru"
    assert "yake_languages:ru" in result.diagnostics


def test_yake_keywords_called_with_mapped_lan(tmp_path: Path) -> None:
    with patch(
        "research_graph.workflows.composition.hybrid_readiness_handoff.yake_keywords_for_text",
        return_value=["граф", "сети"],
    ) as mock_yake:
        run_hybrid_readiness_handoff(
            _handoff_req(tmp_path, _RU_BODY, use_yake_keywords=True)
        )
    assert mock_yake.called
    kwargs = mock_yake.call_args.kwargs
    assert kwargs["language"] == yake_language_code("ru")


def test_non_arxiv_en_yake_language_when_catalog_present() -> None:
    if not BLOG_ARTICLE.is_file():
        return
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
    assert on.preprocess["yake_language"] == "en"
    assert any(d.startswith("yake_language:en") for d in on.diagnostics)
