"""Integrity tests for M121 expanded article catalog.

These tests are intentionally artifact-backed: they validate the real M056
cumulative corpus and the real canonical catalog produced by M121 S02.
They must remain offline and read-only.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from research_graph.infrastructure.corpus.ingestion.catalog_ingest import (
    load_m056_corpus,
    verify_m056_sha256,
)

REPO_ROOT = Path("/root/daily-archive")
CATALOG_ROOT = REPO_ROOT / "data" / "article_catalog" / "article_catalog"
INDEX_PATH = CATALOG_ROOT / "index.json"
EXPECTED_ARTICLE_COUNT = 221
EXPECTED_M056_COUNT = 166


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def _article_path_for_pdf(pdf_path: Path) -> Path:
    return pdf_path.parents[1] / "article.json"


def _m056_articles() -> list[tuple[str, dict[str, Any]]]:
    records = load_m056_corpus(repo_root=REPO_ROOT)
    articles: list[tuple[str, dict[str, Any]]] = []
    for arxiv_id, record in sorted(records.items()):
        article_path = _article_path_for_pdf(record.pdf_path)
        articles.append((arxiv_id, _load_json(article_path)))
    return articles


def test_expanded_catalog_has_221_article_records() -> None:
    article_paths = sorted(CATALOG_ROOT.rglob("article.json"))
    assert len(article_paths) == EXPECTED_ARTICLE_COUNT


def test_all_m056_pdf_hashes_still_match_cumulative_corpus() -> None:
    records = load_m056_corpus(repo_root=REPO_ROOT)
    assert len(records) == EXPECTED_M056_COUNT
    assert verify_m056_sha256(records) == []


def test_all_m056_articles_exist_at_pdf_parent_paths() -> None:
    records = load_m056_corpus(repo_root=REPO_ROOT)
    missing = [
        str(_article_path_for_pdf(record.pdf_path).relative_to(REPO_ROOT))
        for record in records.values()
        if not _article_path_for_pdf(record.pdf_path).exists()
    ]
    assert missing == []


def test_m056_article_identity_matches_cumulative_corpus() -> None:
    records = load_m056_corpus(repo_root=REPO_ROOT)
    for arxiv_id, article in _m056_articles():
        record = records[arxiv_id]
        identity = article["identity"]
        assert article["schema_version"] == "article.v00.01"
        assert article["article_key"] == arxiv_id
        assert article["coarse_topic_code"] == record.category
        assert identity["arxiv_id"] == arxiv_id
        assert identity["sha256"] == record.sha256
        assert identity["size_bytes"] == record.size_bytes
        assert identity["pages_estimate"] == record.pages_estimate
        assert identity["source_milestone"] == record.source_milestone


def test_m056_articles_are_fail_closed_and_synthetic_only() -> None:
    for arxiv_id, article in _m056_articles():
        safety_flags = article["safety_flags"]
        safety_defaults = article["safety_defaults"]
        safety_override = article["safety_override"]
        expected_profile = article["expected_profile"]
        assert safety_override["external_network_authorized"] is False, arxiv_id
        assert safety_defaults["external_network_authorized"] is False, arxiv_id
        assert safety_defaults["graph_writes_authorized"] is False, arxiv_id
        assert safety_defaults["production_import_authorized"] is False, arxiv_id
        assert safety_defaults["llm_calls_authorized"] is False, arxiv_id
        assert safety_flags["graph_import_allowed"] is False, arxiv_id
        assert safety_flags["production_ladybugdb_write_allowed"] is False, arxiv_id
        assert safety_flags["trusted_kg_import_allowed"] is False, arxiv_id
        assert safety_flags["production_import_attempted"] is False, arxiv_id
        assert safety_flags["ladybugdb_written"] is False, arxiv_id
        assert expected_profile["synthetic_metadata"] is True, arxiv_id
        assert expected_profile["graph_ready"] is False, arxiv_id
        assert expected_profile["parser_ready"] is False, arxiv_id
        assert expected_profile["chunk_ready"] is False, arxiv_id


def test_m056_articles_do_not_claim_network_fetches() -> None:
    for arxiv_id, article in _m056_articles():
        assert article["safety_flags"]["network_fetch_required_for_pipeline_phase"] is False
        for variant in article["source_variants"]:
            assert variant["network_fetch_attempted"] is False, (
                arxiv_id,
                variant["variant_id"],
            )


def test_index_json_matches_catalog_articles() -> None:
    index = _load_json(INDEX_PATH)
    articles = index["articles"]
    article_paths = sorted(CATALOG_ROOT.rglob("article.json"))
    indexed_paths = {entry["article_path"] for entry in articles}
    real_paths = {str(path.relative_to(CATALOG_ROOT.parent)) for path in article_paths}
    assert index["schema_version"] == "article-catalog-index.v00.01"
    assert len(articles) == EXPECTED_ARTICLE_COUNT
    assert indexed_paths == real_paths


def test_index_includes_all_m056_article_keys() -> None:
    records = load_m056_corpus(repo_root=REPO_ROOT)
    index = _load_json(INDEX_PATH)
    indexed_keys = {entry["article_key"] for entry in index["articles"]}
    assert set(records) <= indexed_keys
