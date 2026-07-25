from __future__ import annotations

from pathlib import Path

import pytest

from research_graph.application.corpus.catalog_safety import (
    article_ref_from_path,
    normalize_posix_path,
    safe_catalog_path,
    safety_flag_errors,
)

FORBIDDEN_TRUE_FLAGS = {"graph_import_allowed", "ladybugdb_written"}


def test_catalog_paths_normalize_and_stay_under_catalog_root(tmp_path: Path) -> None:
    catalog = tmp_path / "catalog.json"
    catalog.write_text("{}", encoding="utf-8")
    article_path = "article_catalog/arxiv/cs-ai/1234.56789/article.json"

    assert normalize_posix_path(r"article_catalog\\arxiv\\cs-ai\\1234.56789\\article.json") == article_path
    assert safe_catalog_path(catalog, article_path) == (tmp_path / article_path).resolve()

    for unsafe in ("/article.json", "../article.json", "article_catalog/../secret/article.json"):
        with pytest.raises(ValueError):
            safe_catalog_path(catalog, unsafe)


def test_article_ref_from_path_accepts_only_canonical_manifest_paths() -> None:
    assert (
        article_ref_from_path(
            "article_catalog/arxiv/cs-ai/1234.56789/article.json",
            catalog_record_dir="article_catalog",
        )
        == "arxiv/cs-ai/1234.56789"
    )

    for noncanonical in (
        "arxiv/cs-ai/1234.56789/article.json",
        "article_catalog/arxiv/cs-ai/1234.56789/source.pdf",
    ):
        with pytest.raises(ValueError):
            article_ref_from_path(noncanonical, catalog_record_dir="article_catalog")


def test_safety_flag_errors_are_recursive_and_fail_closed() -> None:
    assert (
        safety_flag_errors(
            "catalog",
            {"graph_import_allowed": False, "nested": [{"ladybugdb_written": False}]},
            forbidden_true_flags=FORBIDDEN_TRUE_FLAGS,
        )
        == []
    )

    assert safety_flag_errors(
        "catalog",
        {"nested": [{"graph_import_allowed": True}]},
        forbidden_true_flags=FORBIDDEN_TRUE_FLAGS,
    ) == ["catalog.nested[0].graph_import_allowed must be false; got True"]
