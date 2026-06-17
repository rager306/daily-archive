"""Hypothesis properties for refactored modular article contracts."""

from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any

from hypothesis import given, settings
from hypothesis import strategies as st

from research_graph.papers.artifacts.models import FORBIDDEN_PAYLOAD_KEYS, validate_article_artifact_manifest
from research_graph.papers.source_assets.registry import SourceAssetManifest, validate_source_asset_manifest
from research_graph.papers.chunking.chunker import parse_markdown_structure
from research_graph.identity.canonicalization import canonical_source_id, stable_json_hash
from research_graph.corpus.ingestion.loader import ArticleLoadResult, ArticleLoadSource, FullTextIngestionResult, load_article_source
from research_graph.corpus.parsing.parser import parse_article
from research_graph.staging.import_boundary import ImportCandidate, validate_import_boundary_rehearsal
from tests.helpers.modular_fixtures import (
    FIXTURE_PAPER_ID,
    adaptix_dump,
    sample_article_load_result,
    sample_asset_record,
    sample_import_boundary_rehearsal,
    sample_preserved_source_file,
    sample_redacted_article_structure,
)

SAFE_SLUG = st.from_regex(r"[a-z][a-z0-9]{2,12}", fullmatch=True)
PAPER_IDS = st.from_regex(r"property-paper-[a-f0-9]{4,10}", fullmatch=True)
SECTION_TITLES = st.lists(
    st.text(alphabet=st.characters(whitelist_categories=("Ll", "Lu", "Nd"), whitelist_characters=" -"), min_size=3, max_size=24)
    .map(lambda value: " ".join(value.split()).strip() or "Section"),
    min_size=1,
    max_size=4,
    unique=True,
)
BODY_LINES = st.lists(
    st.text(alphabet=st.characters(whitelist_categories=("Ll", "Lu", "Nd"), whitelist_characters=" .,;:-()"), min_size=8, max_size=80)
    .map(lambda value: " ".join(value.split()).strip() or "deterministic boundary line"),
    min_size=1,
    max_size=4,
)
REFUSAL_REASONS = st.lists(SAFE_SLUG, min_size=0, max_size=3, unique=True).map(tuple)

PROPERTY_SETTINGS = settings(max_examples=25, deadline=None)
FORBIDDEN_RENDERED_KEYS = tuple(f'"{key}"' for key in FORBIDDEN_PAYLOAD_KEYS)
FORBIDDEN_REVIEW_SENTINELS = (
    '"trusted_kg_import_allowed": true',
    '"ladybugdb_written": true',
    '"production_import_attempted": true',
    '"promoted_to_fact": true',
    '"import_eligible": true',
)


def _markdown(title: str, section_titles: list[str], body_lines: list[str]) -> str:
    sections = [f"# {title}\n"]
    for index, heading in enumerate(section_titles):
        body = body_lines[index % len(body_lines)]
        sections.append(f"## {heading}\n{body}\n")
    return "\n".join(sections)


def _assert_no_forbidden_review_payload(payload: dict[str, Any]) -> None:
    rendered = json.dumps(payload, sort_keys=True)
    for forbidden in FORBIDDEN_RENDERED_KEYS:
        assert forbidden not in rendered
    for sentinel in FORBIDDEN_REVIEW_SENTINELS:
        assert sentinel not in rendered


def _to_ingestion(result: ArticleLoadResult) -> FullTextIngestionResult:
    assert result.text is not None
    assert result.quality is not None
    return FullTextIngestionResult(
        paper_id=result.paper_id or result.source_id,
        source_type=result.source_type,
        source_path=result.source_path,
        text=result.text,
        extraction_mode="structured_markdown" if result.source_type == "markdown" else "plain_text",
        warnings=list(result.warnings),
        fallback_reason=result.failure_reason,
        quality=result.quality,
        provenance={str(key): str(value) for key, value in (result.provenance or {}).items() if value is not None},
    )


@PROPERTY_SETTINGS
@given(paper_id=PAPER_IDS, section_titles=SECTION_TITLES, body_lines=BODY_LINES)
def test_loader_provenance_and_parser_outputs_are_deterministic(
    paper_id: str, section_titles: list[str], body_lines: list[str]
) -> None:
    """Local loader provenance and parser output should stay stable at the module boundary."""
    with TemporaryDirectory() as directory:
        source_path = Path(directory) / f"{paper_id}.md"
        markdown = _markdown("Property Fixture", section_titles, body_lines)
        source_path.write_text(markdown, encoding="utf-8")

        first_load = load_article_source(ArticleLoadSource(source_path, paper_id=paper_id, source_type="markdown"))
        second_load = load_article_source(ArticleLoadSource(source_path, paper_id=paper_id, source_type="markdown"))

    assert first_load.outcome == second_load.outcome == "loaded"
    assert first_load.failure_reason is None
    assert first_load.provenance is not None
    assert first_load.provenance["source_id"] == first_load.source_id
    assert first_load.provenance["paper_id"] == paper_id
    assert first_load.provenance["source_path"] == str(source_path)
    assert first_load.provenance["sha256"] == first_load.sha256 == second_load.sha256
    assert first_load.provenance["loader_name"] == "local_article_loader"
    assert first_load.text == second_load.text == markdown.rstrip("\n")

    first_parse = parse_article(_to_ingestion(first_load))
    second_parse = parse_article(_to_ingestion(second_load))

    assert adaptix_dump(first_parse) == adaptix_dump(second_parse)
    assert [element.id for element in first_parse.elements] == [element.id for element in second_parse.elements]
    assert [element.parent_id for element in first_parse.elements] == [element.parent_id for element in second_parse.elements]


@PROPERTY_SETTINGS
@given(section_titles=SECTION_TITLES, body_lines=BODY_LINES)
def test_parser_page_index_and_identity_remain_canonical(section_titles: list[str], body_lines: list[str]) -> None:
    """Parsing and identity helpers should produce stable IDs independent of repeated calls."""
    markdown = _markdown("Identity Fixture", section_titles, body_lines)
    ingestion = _to_ingestion(sample_article_load_result())
    ingestion = FullTextIngestionResult(
        paper_id=ingestion.paper_id,
        source_type=ingestion.source_type,
        source_path=ingestion.source_path,
        text=markdown,
        extraction_mode=ingestion.extraction_mode,
        warnings=ingestion.warnings,
        fallback_reason=ingestion.fallback_reason,
        quality=ingestion.quality,
        provenance=ingestion.provenance,
    )

    first = parse_article(ingestion)
    second = parse_article(ingestion)

    assert adaptix_dump(first) == adaptix_dump(second)
    assert stable_json_hash({"paper_id": FIXTURE_PAPER_ID, "sections": section_titles}) == stable_json_hash(
        {"sections": section_titles, "paper_id": FIXTURE_PAPER_ID}
    )
    assert canonical_source_id(FIXTURE_PAPER_ID, "normalized-md") == canonical_source_id(
        FIXTURE_PAPER_ID, "normalized-md"
    )


@PROPERTY_SETTINGS
@given(paper_id=PAPER_IDS, section_titles=SECTION_TITLES, body_lines=BODY_LINES)
def test_chunking_contract_separates_content_from_asset_metadata(
    paper_id: str, section_titles: list[str], body_lines: list[str]
) -> None:
    """Chunk contracts may expose spans and routes, but asset/review surfaces stay metadata-only."""
    markdown = _markdown("Chunk Fixture", section_titles, body_lines)
    package = parse_markdown_structure(
        markdown,
        paper_id=paper_id,
        title="Chunk Fixture",
        source_artifact=f"tests/fixtures/modular/{paper_id}.md",
        categories=("cs.AI",),
        run_id="property-modular-boundary",
    ).to_contract()

    assert package["paper"]["paper_id"] == paper_id
    assert package["chunks"]
    assert {chunk["paper_id"] for chunk in package["chunks"]} == {paper_id}
    assert all(chunk["source_artifact"].endswith(f"{paper_id}.md") for chunk in package["chunks"])
    assert all(chunk["redaction"]["chunk_text_included"] is False for chunk in package["chunks"])
    assert all(chunk["redaction"]["embeddings_included"] is False for chunk in package["chunks"])
    assert all(chunk["route"] for chunk in package["chunks"])

    asset_manifest = SourceAssetManifest(
        paper_id=FIXTURE_PAPER_ID,
        workspace_root="workspace/modular",
        source_files=(sample_preserved_source_file(),),
        assets=(sample_asset_record(),),
    ).to_contract()
    assert validate_source_asset_manifest(asset_manifest).passed is True
    _assert_no_forbidden_review_payload(asset_manifest)


@PROPERTY_SETTINGS
@given(extra_slug=SAFE_SLUG)
def test_article_artifact_structure_properties_fail_closed(extra_slug: str) -> None:
    """Redacted article structures should generate valid review-only manifests and reject raw payloads."""
    structure = sample_redacted_article_structure()
    extra_artifact_id = f"{FIXTURE_PAPER_ID}:artifact:method:{extra_slug}"
    structure["structured_markers"] = [
        {
            "artifact_id": extra_artifact_id,
            "artifact_type": "method",
            "section_id": f"{FIXTURE_PAPER_ID}:section:methods",
            "span_id": f"{FIXTURE_PAPER_ID}:span:methods",
        }
    ]

    from research_graph.papers.artifacts.models import build_article_artifact_manifest_from_structure

    manifest = build_article_artifact_manifest_from_structure(structure, run_id="property-modular-boundary")

    assert validate_article_artifact_manifest(manifest) == []
    assert manifest["import_eligible_count"] == 0
    assert manifest["promoted_to_fact_count"] == 0
    assert extra_artifact_id in {artifact["artifact_id"] for artifact in manifest["artifacts"]}
    _assert_no_forbidden_review_payload(manifest)

    malformed = deepcopy(structure)
    malformed["artifact_placeholders"][0]["caption_text"] = "raw caption should shrink to this field"
    try:
        build_article_artifact_manifest_from_structure(malformed, run_id="property-modular-boundary")
    except ValueError as exc:
        assert "forbidden raw payload keys" in str(exc)
    else:  # pragma: no cover - property should never permit raw payloads
        raise AssertionError("raw artifact payload was accepted")


@PROPERTY_SETTINGS
@given(import_eligible=st.booleans(), refusal_reasons=REFUSAL_REASONS)
def test_staging_candidate_shape_matches_acceptance_invariant(import_eligible: bool, refusal_reasons: tuple[str, ...]) -> None:
    """A staging candidate is accepted only when eligible and refusal-free; otherwise it remains rejected."""
    candidate = ImportCandidate(
        candidate_id="property:candidate:0001",
        method_id="property:method:0001",
        package_id="benchmark-method:property:method:0001",
        candidate_type="method_candidate",
        route="method_extraction",
        state="ok_for_retrieval_only",
        import_eligible=import_eligible,
        refusal_reasons=refusal_reasons,
        remediation_hints=tuple(f"remediate_{reason}" for reason in refusal_reasons),
    )
    contract = candidate.to_contract()

    assert contract["accepted"] is (import_eligible and not refusal_reasons)
    assert contract["rejected"] is (not contract["accepted"])
    assert contract["import_eligible"] is import_eligible
    assert "trusted_kg_import" not in contract["allowed_uses"]
    assert "trusted_kg_import" in contract["excluded_uses"]
    assert contract["ladybugdb_written"] is False
    assert contract["production_import_attempted"] is False

    rehearsal = sample_import_boundary_rehearsal().to_contract()
    validation = validate_import_boundary_rehearsal(rehearsal)
    assert validation.passed is True
    assert rehearsal["accepted_count"] == 0
    assert rehearsal["rejected_count"] == rehearsal["candidate_count"]
    _assert_no_forbidden_review_payload(rehearsal)


@PROPERTY_SETTINGS
@given(section_titles=SECTION_TITLES)
def test_negative_page_index_navigation_property_shrinks_boundary_mistakes(section_titles: list[str]) -> None:
    """Broken PageIndex next pointers should produce deterministic, local diagnostics."""
    markdown = _markdown(section_titles[0], section_titles, ["navigation boundary line"])
    ingestion = _to_ingestion(sample_article_load_result())
    ingestion = FullTextIngestionResult(
        paper_id=ingestion.paper_id,
        source_type=ingestion.source_type,
        source_path=ingestion.source_path,
        text=markdown,
        extraction_mode=ingestion.extraction_mode,
        warnings=ingestion.warnings,
        fallback_reason=ingestion.fallback_reason,
        quality=ingestion.quality,
        provenance=ingestion.provenance,
    )
    parsed = parse_article(ingestion)

    from research_graph.papers.indexing.parsed_page_index import build_page_index_from_parsed

    page_index = build_page_index_from_parsed(parsed)
    assert page_index.validate_navigation() == []

    if len(page_index.nodes) > 1:
        page_index.nodes[0].next_id = "missing-node"
        diagnostics = page_index.validate_navigation()
        assert diagnostics == [
            f"node {page_index.nodes[0].id} next_id missing-node does not match {page_index.nodes[1].id}"
        ]
