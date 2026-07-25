"""M234 S01: non_arxiv preprocess_rollup symmetry with hybrid hold diagnostics."""

from __future__ import annotations

from pathlib import Path

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


def test_non_arxiv_emits_preprocess_rollup_when_catalog_present() -> None:
    if not BLOG_ARTICLE.is_file():
        return
    result = run_non_arxiv_html_source_proof(
        NonArxivHtmlSourceProofRequest(
            article_json_path=BLOG_ARTICLE,
            repo_root=ROOT,
            use_yake_keywords=False,
        )
    )
    assert result.import_eligible is False
    assert result.preprocess is not None
    assert result.preprocess_rollup["body_count"] == 1
    assert result.preprocess_rollup["drives_verdict"] is False
    assert result.preprocess_rollup["import_eligible"] is False
    assert "token_frequency" in result.preprocess_rollup["keyword_source_counts"]
    assert any(d.startswith("preprocess_rollup_bodies:1") for d in result.diagnostics)
    payload = result.to_dict()
    assert payload["preprocess_rollup"]["body_count"] == 1
    assert payload["import_eligible"] is False


def test_article_without_html_rollup_empty(tmp_path: Path) -> None:
    art = tmp_path / "article.json"
    art.write_text(
        '{"source_code":"company_blog","article_key":"x","paths":{}}',
        encoding="utf-8",
    )
    result = run_non_arxiv_html_source_proof(
        NonArxivHtmlSourceProofRequest(
            article_json_path=art,
            repo_root=tmp_path,
        )
    )
    assert result.import_eligible is False
    assert result.proof_pass is False
    assert result.preprocess is None
    assert result.preprocess_rollup["body_count"] == 0
    assert result.preprocess_rollup["drives_verdict"] is False
    assert any(d.startswith("preprocess_rollup_bodies:0") for d in result.diagnostics)
