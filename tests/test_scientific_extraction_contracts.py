"""Contract tests for S04 scientific extraction drafts.

These tests define Claim, ScientificEntity, ScientificRelation, and
ExtractionPatch contracts before implementation. They consume S03 EvidencePath
fixtures and do not call LLMs, DSPy, embeddings, or LadybugDB.
"""

from __future__ import annotations

from dataclasses import replace
from pathlib import Path

from arxiv_archive.evidence import EvidencePath, build_evidence_path, build_semantic_chunks
from arxiv_archive.full_text import FullTextSource, ingest_full_text
from arxiv_archive.page_index import PageIndexDocument, build_page_index
from arxiv_archive.scientific_extraction import (
    Claim,
    ExtractionPatch,
    ScientificEntity,
    ScientificRelation,
    claim_id,
    entity_id,
    validate_claim,
    validate_extraction_patch,
)

FULL_TEXT_FIXTURES = Path(__file__).parent / "fixtures" / "full_text"
SCHEMA_VERSION = "scientific_extraction.v1"
EXTRACTOR_VERSION = "fixture-extractor.v1"


def build_document() -> PageIndexDocument:
    ingestion = ingest_full_text(
        FullTextSource(
            paper_id="2605.12345",
            source_type="markdown",
            source_path=FULL_TEXT_FIXTURES / "structured_paper.md",
        )
    )
    return build_page_index(ingestion)


def method_evidence_path() -> EvidencePath:
    document = build_document()
    chunks = build_semantic_chunks(document)
    method = next(chunk for chunk in chunks if chunk.page_index_node_id == "2605.12345:method")
    return build_evidence_path(document, method)


def sample_claim(evidence: EvidencePath | None = None, *, paper_id: str = "2605.12345") -> Claim:
    return Claim(
        id="claim:2605.12345:method:chunk-0001:local-markdown-pageindex",
        paper_id=paper_id,
        text="Local markdown is enough to build a deterministic PageIndex.",
        claim_type="method",
        confidence=0.91,
        evidence_path=evidence if evidence is not None else method_evidence_path(),
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "fixture"},
    )


def sample_entity(evidence: EvidencePath | None = None) -> ScientificEntity:
    return ScientificEntity(
        id="entity:2605.12345:pageindex",
        paper_id="2605.12345",
        label="PageIndex",
        entity_type="method",
        confidence=0.88,
        evidence_path=evidence if evidence is not None else method_evidence_path(),
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "fixture"},
    )


def sample_relation(
    claim: Claim,
    entity: ScientificEntity,
    evidence: EvidencePath | None = None,
    *,
    relation_type: str = "supports",
    target_id: str | None = None,
) -> ScientificRelation:
    return ScientificRelation(
        id=f"relation:2605.12345:claim-local-markdown-pageindex:entity-pageindex:{relation_type}",
        paper_id="2605.12345",
        relation_type=relation_type,
        source_id=claim.id,
        target_id=target_id or entity.id,
        confidence=0.84,
        evidence_path=evidence if evidence is not None else method_evidence_path(),
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "fixture"},
    )


def sample_patch(
    claim: Claim | None = None,
    entity: ScientificEntity | None = None,
    relation: ScientificRelation | None = None,
) -> ExtractionPatch:
    evidence = method_evidence_path()
    claim = claim or sample_claim(evidence)
    entity = entity or sample_entity(evidence)
    relation = relation or sample_relation(claim, entity, evidence)
    return ExtractionPatch(
        paper_id="2605.12345",
        claims=[claim],
        entities=[entity],
        relations=[relation],
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "fixture"},
    )


def test_scientific_ids_use_parser_normalization_contract() -> None:
    assert claim_id("2605.12345", "2605.12345:method:chunk-0001", "Local Markdown PageIndex") == (
        "claim:2605.12345:2605-12345-method-chunk-0001:local-markdown-pageindex"
    )
    assert entity_id("2605.12345", "PageIndex") == "entity:2605.12345:pageindex"


def test_claim_entity_relation_models_are_storage_ready_and_traceable() -> None:
    evidence = method_evidence_path()
    claim = sample_claim(evidence)
    entity = sample_entity(evidence)
    relation = sample_relation(claim, entity, evidence)

    assert claim.evidence_path is not None
    assert claim.evidence_path.semantic_chunk_id == "2605.12345:method:chunk-0001"
    assert entity.evidence_path is not None
    assert entity.evidence_path.node_path == ["2605.12345:root", "2605.12345:method"]
    assert relation.source_id == claim.id
    assert relation.target_id == entity.id
    assert relation.relation_type == "supports"
    assert validate_claim(claim) == []


def test_claim_validation_reports_missing_evidence_invalid_confidence_and_versions() -> None:
    claim = Claim(
        id="claim:2605.12345:bad",
        paper_id="2605.12345",
        text="Bad claim",
        claim_type="method",
        confidence=1.4,
        evidence_path=None,
        schema_version="",
        extractor_version="",
        validation_warnings=[],
        provenance={},
    )

    assert validate_claim(claim) == [
        "Claim claim:2605.12345:bad is missing evidence_path",
        "Claim claim:2605.12345:bad confidence 1.4 is outside [0.0, 1.0]",
        "Claim claim:2605.12345:bad is missing schema_version",
        "Claim claim:2605.12345:bad is missing extractor_version",
        "Claim claim:2605.12345:bad is missing provenance",
    ]


def test_extraction_patch_groups_claims_entities_relations_with_versions() -> None:
    evidence = method_evidence_path()
    claim = sample_claim(evidence)
    entity = sample_entity(evidence)
    relation = sample_relation(claim, entity, evidence)
    patch = sample_patch(claim, entity, relation)

    assert patch.paper_id == "2605.12345"
    assert patch.schema_version == SCHEMA_VERSION
    assert patch.extractor_version == EXTRACTOR_VERSION
    assert [item.id for item in patch.claims] == [claim.id]
    assert [item.id for item in patch.entities] == [entity.id]
    assert [item.id for item in patch.relations] == [relation.id]
    assert validate_extraction_patch(patch) == []


def test_patch_validation_reports_invalid_relation_endpoint_and_paper_mismatch() -> None:
    evidence = method_evidence_path()
    claim = sample_claim(evidence, paper_id="2605.99999")
    relation = sample_relation(claim, sample_entity(evidence), evidence, target_id="entity:2605.12345:missing")
    patch = sample_patch(claim=claim, entity=None, relation=relation)
    patch = replace(patch, entities=[])

    diagnostics = validate_extraction_patch(patch)

    assert (
        "Claim claim:2605.12345:method:chunk-0001:local-markdown-pageindex paper_id 2605.99999 "
        "does not match patch paper_id 2605.12345"
    ) in diagnostics
    assert (
        "Relation relation:2605.12345:claim-local-markdown-pageindex:entity-pageindex:supports "
        "target_id entity:2605.12345:missing does not reference a claim or entity in the patch"
    ) in diagnostics


def test_patch_validation_reports_unsupported_relation_type_and_duplicate_ids() -> None:
    evidence = method_evidence_path()
    claim = sample_claim(evidence)
    duplicate_claim = replace(claim, text="Duplicate claim text.")
    entity = sample_entity(evidence)
    relation = sample_relation(claim, entity, evidence, relation_type="causes")
    patch = replace(sample_patch(claim, entity, relation), claims=[claim, duplicate_claim])

    diagnostics = validate_extraction_patch(patch)

    assert (
        "Relation relation:2605.12345:claim-local-markdown-pageindex:entity-pageindex:causes "
        "relation_type causes is unsupported"
    ) in diagnostics
    assert "ExtractionPatch 2605.12345 has duplicate draft id claim:2605.12345:method:chunk-0001:local-markdown-pageindex" in diagnostics


def test_patch_validation_reports_evidence_path_warnings() -> None:
    evidence = replace(method_evidence_path(), validation_warnings=["evidence path references missing SemanticChunk missing"])
    claim = sample_claim(evidence)
    entity = sample_entity(evidence)
    relation = sample_relation(claim, entity, evidence)
    patch = sample_patch(claim, entity, relation)

    diagnostics = validate_extraction_patch(patch)

    assert (
        "Claim claim:2605.12345:method:chunk-0001:local-markdown-pageindex evidence_path has validation warnings: "
        "evidence path references missing SemanticChunk missing"
    ) in diagnostics
    assert (
        "ScientificEntity entity:2605.12345:pageindex evidence_path has validation warnings: "
        "evidence path references missing SemanticChunk missing"
    ) in diagnostics
    assert (
        "Relation relation:2605.12345:claim-local-markdown-pageindex:entity-pageindex:supports "
        "evidence_path has validation warnings: evidence path references missing SemanticChunk missing"
    ) in diagnostics
