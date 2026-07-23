"""M225 S03: package enrichment with language and outline."""

from __future__ import annotations

from research_graph.application.corpus.article_preprocess import (
    SCHEMA_VERSION,
    build_article_preprocess_package,
)


def test_package_includes_language_and_outline() -> None:
    text = (
        "# Introduction\n\n"
        "The method and results of this experiment show that graph neural networks "
        "are effective for citation prediction tasks.\n\n"
        "## Methods\n\n"
        "We evaluate message passing on citation graphs and molecular graphs.\n"
    )
    pkg = build_article_preprocess_package(
        source_id="paper-en",
        text=text,
        source_class="arxiv_pdf",
        profile="scholarly",
    )
    assert SCHEMA_VERSION.startswith("m225")
    assert pkg.language == "en"
    assert pkg.language_confidence > 0.0
    assert pkg.outline_heading_count >= 2
    assert any(h == "Introduction" for h in pkg.outline_heading_titles)
    assert pkg.import_eligible is False
    d = pkg.to_dict()
    assert d["language"] == "en"
    assert d["outline_heading_count"] >= 2
    assert d["import_eligible"] is False


def test_russian_package_language() -> None:
    text = (
        "Метод и результаты эксперимента показывают, что графовые нейронные сети "
        "эффективны для задач предсказания цитирования и свойств молекул. "
        "Дополнительный текст для устойчивой эвристики языка и качества."
    )
    pkg = build_article_preprocess_package(
        source_id="paper-ru",
        text=text,
        profile="scholarly",
    )
    assert pkg.language == "ru"
