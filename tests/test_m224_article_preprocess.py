"""M224 S03: HTML main-content strip + ArticlePreprocessPackage."""

from __future__ import annotations

from research_graph.application.corpus.article_preprocess import (
    ArticlePreprocessPackage,
    build_article_preprocess_package,
)
from research_graph.application.corpus.html_main_content import extract_html_main_content


CHROME_HTML = """<!DOCTYPE html>
<html><body>
<nav>Home About Subscribe now Cart</nav>
<aside>Promotional banner privacy policy</aside>
<article>
  <h1>Real Article Title</h1>
  <p>Graph neural networks pass messages along edges between nodes.</p>
  <p>Citation graphs and molecules are common application domains for GNN models.</p>
</article>
<footer>Copyright 2026 All rights reserved newsletter signup</footer>
</body></html>
"""


def test_extract_prefers_article_over_chrome() -> None:
    result = extract_html_main_content(CHROME_HTML)
    assert "Real Article Title" in result.text
    assert "Graph neural networks" in result.text
    assert "Subscribe now" not in result.text
    assert "newsletter signup" not in result.text
    assert result.main_content_ratio is not None
    assert result.main_content_ratio > 0.3
    assert result.import_eligible is False


def test_extract_falls_back_to_body_without_article() -> None:
    html = "<html><body><p>Only body paragraph about methods and results.</p></body></html>"
    result = extract_html_main_content(html)
    assert "methods and results" in result.text
    assert result.region in {"body", "full"}


def test_build_package_from_plain_text_scholarly() -> None:
    text = " ".join(["method", "result", "graph", "neural"] * 30)
    pkg = build_article_preprocess_package(
        source_id="paper-1",
        text=text,
        source_class="arxiv_pdf",
        profile="scholarly",
    )
    assert pkg.source_id == "paper-1"
    assert pkg.cleaned_text
    assert "normalize_unicode" in pkg.clean_ops
    assert pkg.quality_status in {"ok", "soft_signal"}
    assert pkg.import_eligible is False
    assert pkg.graph_writes_allowed is False
    assert pkg.html_main_content_ratio is None
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["schema_version"].startswith("m224")


def test_build_package_from_html_strips_chrome() -> None:
    pkg = build_article_preprocess_package(
        source_id="blog-1",
        text=CHROME_HTML,
        source_class="company_blog",
        profile="web",
        is_html=True,
    )
    assert "Subscribe now" not in pkg.cleaned_text
    assert "Graph neural networks" in pkg.cleaned_text
    assert pkg.html_main_content_ratio is not None
    assert "html_main_content" in pkg.clean_ops
    assert pkg.import_eligible is False


def test_package_rejects_import_true() -> None:
    import pytest

    with pytest.raises(ValueError, match="import"):
        ArticlePreprocessPackage(
            schema_version="m224-article-preprocess.v1",
            source_id="x",
            source_class="web",
            profile="web",
            cleaned_text="hello",
            clean_ops=(),
            quality_status="ok",
            quality_rule_hits=(),
            quality_scores={},
            word_count=1,
            import_eligible=True,
        )
