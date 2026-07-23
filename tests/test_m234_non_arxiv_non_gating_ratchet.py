"""M234 S02: non_arxiv preprocess must not gate proof_pass or import."""

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


def test_yake_flag_does_not_change_proof_pass_or_import() -> None:
    if not BLOG_ARTICLE.is_file():
        return
    off = run_non_arxiv_html_source_proof(
        NonArxivHtmlSourceProofRequest(
            article_json_path=BLOG_ARTICLE,
            repo_root=ROOT,
            use_yake_keywords=False,
        )
    )
    on = run_non_arxiv_html_source_proof(
        NonArxivHtmlSourceProofRequest(
            article_json_path=BLOG_ARTICLE,
            repo_root=ROOT,
            use_yake_keywords=True,
        )
    )
    assert off.import_eligible is False
    assert on.import_eligible is False
    assert off.proof_pass == on.proof_pass
    assert off.preprocess is not None and on.preprocess is not None
    assert off.preprocess["keyword_source"] == "token_frequency"
    assert on.preprocess["keyword_source"] == "injected"
    assert off.preprocess_rollup["drives_verdict"] is False
    assert on.preprocess_rollup["drives_verdict"] is False


def test_high_min_body_chars_fails_proof_not_import() -> None:
    if not BLOG_ARTICLE.is_file():
        return
    # Force proof_pass false via threshold without touching preprocess quality.
    result = run_non_arxiv_html_source_proof(
        NonArxivHtmlSourceProofRequest(
            article_json_path=BLOG_ARTICLE,
            repo_root=ROOT,
            min_body_chars=10**12,
            use_yake_keywords=False,
        )
    )
    assert result.proof_pass is False
    assert result.import_eligible is False
    # Preprocess may still enrich when body loaded.
    if result.preprocess is not None:
        assert result.preprocess["import_eligible"] is False
        assert result.preprocess_rollup["import_eligible"] is False
        assert result.preprocess_rollup["drives_verdict"] is False
