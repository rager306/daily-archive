from __future__ import annotations

import json
from pathlib import Path
from typing import Any

FIXTURE_DIR = Path(__file__).parent / "fixtures" / "article_catalog_v00_01"
CATALOG_PATH = FIXTURE_DIR / "catalog.json"
CATALOG_INDEX_PATH = FIXTURE_DIR / "article_catalog" / "index.json"
SELECTION_PATH = FIXTURE_DIR / "corpora" / "m025-rlm-dspy-pageindex-smoke-v1" / "selection.json"
ARTICLES_DIR = FIXTURE_DIR / "article_catalog"

EXPECTED_ARTICLES = {
    "arxiv/cs-ai/2512.24601": {
        "source_code": "arxiv",
        "coarse_topic_code": "cs-ai",
        "primary_role": "arxiv_html",
        "must_have_roles": {"arxiv_abs_page", "arxiv_html", "arxiv_pdf"},
    },
    "arxiv/cs-ai/2605.28617v1": {
        "source_code": "arxiv",
        "coarse_topic_code": "cs-ai",
        "primary_role": "arxiv_html",
        "must_have_roles": {"arxiv_html", "arxiv_pdf"},
    },
    "arxiv/cs-cv/2605.26525v1": {
        "source_code": "arxiv",
        "coarse_topic_code": "cs-cv",
        "primary_role": "arxiv_html",
        "must_have_roles": {"arxiv_html", "arxiv_pdf"},
    },
    "arxiv/cs-cl/2507.19457": {
        "source_code": "arxiv",
        "coarse_topic_code": "cs-cl",
        "primary_role": "arxiv_html",
        "must_have_roles": {"arxiv_abs_page", "arxiv_html", "arxiv_pdf"},
    },
    "company_blog/cs-ir/pageindex_zhang2025pageindex": {
        "source_code": "company_blog",
        "coarse_topic_code": "cs-ir",
        "primary_role": "web_article_html",
        "must_have_roles": {"web_article_html", "bibtex_citation"},
    },
}

FORBIDDEN_METADATA_FLAGS = {
    "raw_text_embedded",
    "raw_binary_embedded",
    "trusted_kg_import_allowed",
    "production_ladybugdb_write_allowed",
}


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _article_path(article_ref: str) -> Path:
    return ARTICLES_DIR / article_ref / "article.json"


def _variants_by_role(article: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {str(variant["source_role"]): variant for variant in article["source_variants"]}


def test_article_catalog_schema_v0001_registers_reusable_multi_source_store() -> None:
    catalog = _load_json(CATALOG_PATH)

    assert catalog["schema_version"] == "article-catalog.v00.01"
    assert catalog["article_schema_version"] == "article.v00.01"
    assert catalog["path_template"] == "{source_code}/{coarse_topic_code}/{article_key}"
    assert catalog["index"]["schema_version"] == "article-catalog-index.v00.01"
    assert catalog["index"]["path"] == "index.json"
    assert catalog["index"]["cli_must_use_index"] is True
    assert catalog["index"]["full_tree_scan_allowed"] is False
    assert catalog["index"]["refresh_command_rebuilds_index"] is True
    assert set(catalog["index"]["lookup_keys"]) >= {
        "article_key",
        "citation_key",
        "canonical_url",
        "source_code",
        "coarse_topic_code",
        "title",
    }
    assert catalog["storage_policy"]["raw_sources_captured_once"] is True
    assert catalog["storage_policy"]["pipeline_tests_must_not_fetch_network"] is True
    assert catalog["source_strategy_defaults"]["pdf_policy"] == "capture_immediately_use_as_fallback"
    assert catalog["source_strategy_defaults"]["fallback_policy"] == (
        "use_pdf_when_html_or_markdown_missing_low_quality_or_inconsistent"
    )

    source_codes = {source["source_code"] for source in catalog["sources"]}
    assert {"arxiv", "company_blog", "personal_blog", "nature"} <= source_codes

    topic_codes = set(catalog["topic_code_system"]["allowed_coarse_topic_codes"])
    assert {"cs-ai", "cs-cl", "cs-cv", "cs-ir", "cs-lg", "stat-ml"} <= topic_codes

    safety = catalog["safety_flags"]
    assert safety["metadata_manifests_embed_raw_text"] is False
    assert safety["metadata_manifests_embed_raw_binary"] is False
    assert safety["graph_import_allowed"] is False
    assert safety["production_ladybugdb_write_allowed"] is False


def test_article_catalog_index_v0001_supports_cli_lookup_without_tree_scan() -> None:
    index = _load_json(CATALOG_INDEX_PATH)

    assert index["schema_version"] == "article-catalog-index.v00.01"
    assert index["catalog_schema_version"] == "article-catalog.v00.01"
    assert index["article_schema_version"] == "article.v00.01"
    assert index["lookup_policy"]["cli_must_use_index"] is True
    assert index["lookup_policy"]["full_tree_scan_allowed"] is False
    assert index["lookup_policy"]["refresh_command_rebuilds_index"] is True

    article_refs = {entry["article_ref"] for entry in index["articles"]}
    assert article_refs == set(EXPECTED_ARTICLES)
    assert set(index["indexes"]["by_article_key"].values()) == set(EXPECTED_ARTICLES)
    assert index["indexes"]["by_citation_key"] == {
        "zhang2025pageindex": "company_blog/cs-ir/pageindex_zhang2025pageindex"
    }
    assert index["indexes"]["by_canonical_url"]["https://arxiv.org/abs/2512.24601"] == (
        "arxiv/cs-ai/2512.24601"
    )
    assert index["indexes"]["by_canonical_url"]["https://pageindex.ai/blog/pageindex-intro"] == (
        "company_blog/cs-ir/pageindex_zhang2025pageindex"
    )
    assert index["indexes"]["by_title"]["Recursive Language Models"] == "arxiv/cs-ai/2512.24601"
    assert index["indexes"]["by_title"][
        "PageIndex: Next-Generation Vectorless, Reasoning-based RAG"
    ] == "company_blog/cs-ir/pageindex_zhang2025pageindex"
    assert index["indexes"]["by_source_code"]["arxiv"] == [
        "arxiv/cs-ai/2512.24601",
        "arxiv/cs-ai/2605.28617v1",
        "arxiv/cs-cv/2605.26525v1",
        "arxiv/cs-cl/2507.19457",
    ]
    assert index["indexes"]["by_coarse_topic_code"]["cs-ir"] == [
        "company_blog/cs-ir/pageindex_zhang2025pageindex"
    ]

    for entry in index["articles"]:
        article = _load_json(FIXTURE_DIR / entry["article_path"])
        assert entry["title"] == article["identity"]["title"]
        assert entry["title"]
        assert entry["article_path"].endswith("/article.json")
        assert (FIXTURE_DIR / entry["article_path"]).exists(), entry["article_path"]
        assert entry["primary_source_role"]
        assert isinstance(entry["content_fallback_roles"], list)
        assert isinstance(entry["metadata_roles"], list)


def test_m025_selection_references_catalog_entries_not_live_urls_as_primary_inputs() -> None:
    selection = _load_json(SELECTION_PATH)

    assert selection["schema_version"] == "article-corpus-selection.v00.01"
    assert selection["catalog_schema_version"] == "article-catalog.v00.01"
    assert selection["article_schema_version"] == "article.v00.01"
    assert selection["selection_mode"] == "manual_url_seed"
    assert selection["network_policy"]["capture_phase_may_fetch"] is True
    assert selection["network_policy"]["test_phase_must_not_fetch"] is True

    article_refs = {article["article_ref"] for article in selection["articles"]}
    assert article_refs == set(EXPECTED_ARTICLES)
    for article_ref in article_refs:
        assert _article_path(article_ref).exists(), article_ref

    pageindex = next(
        article for article in selection["articles"] if article["article_ref"].startswith("company_blog/")
    )
    assert pageindex["source_code"] == "company_blog"
    assert pageindex["selection_role"] == "non_arxiv_html_and_bibtex_control"


class TestArticleSchemaV0001:
    def test_selected_articles_follow_source_topic_article_hierarchy(self) -> None:
        for article_ref, expected in EXPECTED_ARTICLES.items():
            article = _load_json(_article_path(article_ref))

            assert article["schema_version"] == "article.v00.01"
            assert article["catalog_path"] == article_ref
            assert article["source_code"] == expected["source_code"]
            assert article["coarse_topic_code"] == expected["coarse_topic_code"]
            assert article["article_key"] == article_ref.rsplit("/", 1)[-1]
            assert article["safety_flags"]["trusted_kg_import_allowed"] is False
            assert article["safety_flags"]["production_ladybugdb_write_allowed"] is False

    def test_source_variants_separate_capability_capture_and_loader_outcome(self) -> None:
        for article_ref, expected in EXPECTED_ARTICLES.items():
            article = _load_json(_article_path(article_ref))
            variants = _variants_by_role(article)

            assert expected["must_have_roles"] <= set(variants), article_ref
            primary_id = article["source_strategy"]["primary_source_variant_id"]
            primary_variant = next(
                variant for variant in article["source_variants"] if variant["variant_id"] == primary_id
            )
            assert primary_variant["source_role"] == expected["primary_role"]
            assert primary_variant["is_primary"] is True
            assert primary_variant["is_content_bearing"] is True
            assert primary_variant["is_metadata_only"] is False

            for variant in article["source_variants"]:
                assert variant["variant_id"].startswith(f"{article['article_key']}:source:")
                assert variant["source_format"] in {
                    "html",
                    "pdf",
                    "markdown",
                    "text",
                    "ocr_text",
                    "bibtex",
                    "api_metadata",
                }
                assert variant["capture_status"] in {"planned", "captured", "failed"}
                assert variant["loader_outcome"] in {
                    "pending",
                    "loaded",
                    "loaded_metadata_only",
                    "low_quality_source",
                    "failed",
                    "unsupported",
                }
                assert variant["raw_text_embedded"] is False
                assert variant["raw_binary_embedded"] is False

    def test_arxiv_articles_capture_pdf_immediately_but_prefer_html_when_available(self) -> None:
        for article_ref in [ref for ref in EXPECTED_ARTICLES if ref.startswith("arxiv/")]:
            article = _load_json(_article_path(article_ref))
            variants = _variants_by_role(article)
            strategy = article["source_strategy"]

            assert strategy["preferred_content_order"][0] == "arxiv_html"
            assert strategy["pdf_policy"] == "capture_immediately_use_as_fallback"
            assert strategy["fallback_policy"] == (
                "use_pdf_when_html_or_markdown_missing_low_quality_or_inconsistent"
            )

            pdf = variants["arxiv_pdf"]
            assert pdf["source_format"] == "pdf"
            assert pdf["is_content_bearing"] is True
            assert pdf["requires_conversion"] is True
            assert pdf["conversion_hint"] == "docling_or_marker_pdf_to_markdown"
            assert pdf["capture_policy"] == "capture_during_acquisition_phase"
            assert pdf["loader_outcome"] == "pending"

            html = variants["arxiv_html"]
            assert html["source_format"] == "html"
            assert html["requires_conversion"] is False
            assert html["capture_policy"] == "capture_during_acquisition_phase"

    def test_company_blog_article_has_html_primary_bibtex_metadata_and_no_pdf_requirement(self) -> None:
        article = _load_json(
            _article_path("company_blog/cs-ir/pageindex_zhang2025pageindex")
        )
        variants = _variants_by_role(article)
        strategy = article["source_strategy"]

        assert article["identity"]["citation_key"] == "zhang2025pageindex"
        assert article["identity"]["container"] == "PageIndex Blog"
        assert strategy["preferred_content_order"] == ["web_article_html"]
        assert strategy["metadata_order"] == ["bibtex_citation", "html_meta", "manual"]
        assert strategy["pdf_policy"] == "no_pdf_expected"
        assert "publisher_pdf" not in variants
        assert "web_article_html" in variants
        assert "bibtex_citation" in variants
        assert variants["bibtex_citation"]["is_metadata_only"] is True
        assert variants["bibtex_citation"]["is_content_bearing"] is False

    def test_metadata_records_do_not_authorize_import_or_embed_raw_payloads(self) -> None:
        serialized = json.dumps(
            {
                "catalog": _load_json(CATALOG_PATH),
                "index": _load_json(CATALOG_INDEX_PATH),
                "selection": _load_json(SELECTION_PATH),
                "articles": [
                    _load_json(_article_path(article_ref)) for article_ref in EXPECTED_ARTICLES
                ],
            },
            sort_keys=True,
        )

        forbidden_snippets = [
            "trusted_kg_import_allowed\": true",
            "production_ladybugdb_write_allowed\": true",
            "raw_text_embedded\": true",
            "raw_binary_embedded\": true",
            "production_import_attempted\": true",
            "ladybugdb_written\": true",
        ]
        for snippet in forbidden_snippets:
            assert snippet not in serialized
