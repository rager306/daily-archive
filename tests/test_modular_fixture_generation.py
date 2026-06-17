"""Tests for shared Adaptix modular fixture generation."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.papers.artifacts.models import FORBIDDEN_PAYLOAD_KEYS
from research_graph.papers.indexing.navigation import PageIndexDocument
from research_graph.corpus.ingestion.loader import ArticleLoadResult
from research_graph.corpus.parsing.structure import ParsedArticle
from tests.helpers.modular_fixtures import (
    FIXTURE_PAPER_ID,
    MODULAR_FIXTURE_PATH,
    adaptix_dump,
    adaptix_load,
    canonical_contract_samples,
    load_canonical_contract_samples,
    sample_article_load_result,
    sample_page_index_document,
    sample_parsed_article,
    sample_redacted_article_structure,
    sample_structure_aware_package,
    write_canonical_contract_samples,
)


def test_shared_adaptix_helpers_roundtrip_loader_parser_and_page_index() -> None:
    """Canonical module fixtures must survive Adaptix dump/load boundaries."""
    loader = sample_article_load_result()
    parsed = sample_parsed_article()
    page_index = sample_page_index_document()

    restored_loader = adaptix_load(adaptix_dump(loader), ArticleLoadResult)
    restored_parsed = adaptix_load(adaptix_dump(parsed), ParsedArticle)
    restored_page_index = adaptix_load(adaptix_dump(page_index), PageIndexDocument)

    assert restored_loader.paper_id == FIXTURE_PAPER_ID
    assert restored_loader.warning_count == 0
    assert restored_loader.quality is not None
    assert restored_loader.quality.status == "ok"
    assert restored_parsed.paper_id == FIXTURE_PAPER_ID
    assert [element.title for element in restored_parsed.elements] == [
        "Fixture Paper",
        "Methods",
        "Results",
    ]
    assert restored_page_index.paper_id == FIXTURE_PAPER_ID
    assert restored_page_index.validate_navigation() == []
    assert [node.id for node in restored_page_index.walk_next()] == [node.id for node in page_index.nodes]


def test_canonical_contract_samples_cover_refactored_module_surfaces() -> None:
    """The shared fixture payload should expose every planned modular contract family."""
    samples = canonical_contract_samples()

    assert set(samples) >= {
        "loader",
        "parser",
        "page_index",
        "chunking_contract",
        "asset_source",
        "asset_record",
        "identity",
        "article_artifacts",
        "article_page_index",
        "staging",
    }
    assert samples["paper_id"] == FIXTURE_PAPER_ID
    assert samples["chunking_contract"]["paper"]["paper_id"] == FIXTURE_PAPER_ID
    assert samples["article_artifacts"]["import_eligible_count"] == 0
    assert samples["article_page_index"]["paper_id"] == FIXTURE_PAPER_ID
    assert samples["staging"]["accepted_count"] == 0
    assert samples["staging"]["rejected_count"] == 1


def test_fixture_generation_persists_stable_json(tmp_path: Path) -> None:
    """Fixture generation is reproducible and visible as deterministic JSON."""
    first_path = write_canonical_contract_samples(tmp_path / "canonical_contract_samples.json")
    first_payload = first_path.read_text(encoding="utf-8")
    second_path = write_canonical_contract_samples(tmp_path / "canonical_contract_samples.json")
    second_payload = second_path.read_text(encoding="utf-8")

    assert first_path == second_path
    assert first_payload == second_payload
    loaded = json.loads(first_payload)
    assert loaded["schema_version"] == "modular-fixture-samples.v1"
    assert loaded["identity"]["canonical_source_id"].startswith("source-fixture-paper-modular-0001")


def test_checked_in_modular_fixture_matches_generated_payload() -> None:
    """The tracked modular fixture should match helper-generated canonical samples."""
    generated = canonical_contract_samples()
    persisted = load_canonical_contract_samples(MODULAR_FIXTURE_PATH)

    assert persisted == generated


def test_modular_fixtures_remain_metadata_only_and_fail_closed() -> None:
    """Review/import-facing generated samples must not smuggle raw payloads or authorization."""
    samples = canonical_contract_samples()
    review_only_payload = {
        key: samples[key]
        for key in (
            "chunking_contract",
            "asset_source",
            "asset_record",
            "article_artifacts",
            "article_page_index",
            "staging",
        )
    }
    rendered = json.dumps(review_only_payload, sort_keys=True)

    for forbidden_key in FORBIDDEN_PAYLOAD_KEYS:
        assert f'"{forbidden_key}"' not in rendered
    assert '"trusted_kg_import_allowed": true' not in rendered
    assert '"ladybugdb_written": true' not in rendered
    assert '"production_import_attempted": true' not in rendered
    assert samples["article_artifacts"]["safety_flags"]["raw_text_included"] is False
    assert samples["asset_record"]["promoted_to_fact"] is False
    assert samples["asset_record"]["excluded_uses"]


def test_negative_malformed_structure_fixture_bubbles_validation_error() -> None:
    """Malformed shared structures should fail loudly instead of generating unsafe fixtures."""
    malformed = sample_redacted_article_structure()
    malformed["raw_text"] = "raw prose must never be present in this fixture family"

    from research_graph.papers.artifacts.models import build_article_artifact_manifest_from_structure

    with pytest.raises(ValueError, match="forbidden raw payload keys"):
        build_article_artifact_manifest_from_structure(malformed)


def test_negative_page_index_navigation_fixture_detects_broken_next_pointer() -> None:
    """Boundary mistakes in generated PageIndex samples should remain observable."""
    page_index = sample_page_index_document()
    page_index.nodes[0].next_id = "missing-node"

    diagnostics = page_index.validate_navigation()

    assert diagnostics == [
        f"node {page_index.nodes[0].id} next_id missing-node does not match {page_index.nodes[1].id}"
    ]


def test_structure_aware_package_fixture_has_10x_bounded_shape() -> None:
    """The canonical helper keeps fixture load linear and small under repeated generation."""
    packages = [sample_structure_aware_package().to_contract() for _ in range(10)]
    chunk_counts = [len(package["chunks"]) for package in packages]

    assert chunk_counts == [chunk_counts[0]] * 10
    assert chunk_counts[0] > 0
    assert sum(chunk_counts) <= 10 * 8
