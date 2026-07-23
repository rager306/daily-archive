"""M226 S02: preprocess enrichment on non-arxiv HTML proof (import-blocked)."""

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


def test_proof_includes_preprocess_enrichment() -> None:
    if not BLOG_ARTICLE.is_file():
        return
    result = run_non_arxiv_html_source_proof(
        NonArxivHtmlSourceProofRequest(
            article_json_path=BLOG_ARTICLE,
            repo_root=ROOT,
            min_body_chars=500,
            min_chunks=1,
        )
    )
    assert result.import_eligible is False
    assert result.hybrid_claimed_success is False
    assert result.proof_pass is True
    assert result.preprocess is not None
    assert result.preprocess["import_eligible"] is False
    assert result.preprocess["content_fingerprint_sha256"]
    assert len(result.preprocess["content_fingerprint_sha256"]) == 64
    assert result.preprocess["language"]
    assert "quality_status" in result.preprocess
    assert result.schema_version.startswith("m226")
    payload = result.to_dict()
    assert payload["import_eligible"] is False
    assert payload["preprocess"]["content_fingerprint_sha256"]
