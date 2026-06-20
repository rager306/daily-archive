"""Contract tests for S05 LadybugDB scientific KG persistence.

These tests define the storage boundary that consumes S02 PageIndex, S03
SemanticChunk/EvidencePath, and S04 ExtractionPatch fixtures. They do not test
retrieval, DSPy, RLM, live LLMs, or production corpus migration.
"""

from __future__ import annotations

from dataclasses import replace
from pathlib import Path
from typing import Any, cast

import ladybug
import pytest

import research_graph.graph.ladybug_client as ladybug_client
from research_graph.corpus.ingestion import FullTextSource, ingest_full_text
from research_graph.evaluation.scientific_extraction import (
    Claim,
    ExtractionPatch,
    ScientificEntity,
    ScientificRelation,
)
from research_graph.graph.ladybug_client import init_db
from research_graph.papers.indexing import PageIndexDocument, build_page_index
from research_graph.papers.semantic_chunks import (
    EvidencePath,
    build_evidence_path,
    build_semantic_chunks,
)

FULL_TEXT_FIXTURES = Path(__file__).parent / "fixtures" / "full_text"
SCHEMA_VERSION = "typed.v1"
EXTRACTOR_VERSION = "fixture-extractor.v1"


def _rows(result: Any) -> list[list[Any]]:
    rows: list[list[Any]] = []
    while result.has_next():
        rows.append(list(result.get_next()))
    return rows


def _scalar(conn: ladybug.Connection, query: str) -> Any:
    result = cast(Any, conn.execute(query))
    assert result.has_next()
    return result.get_next()[0]


def build_document() -> PageIndexDocument:
    ingestion = ingest_full_text(
        FullTextSource(
            paper_id="2605.12345",
            source_type="markdown",
            source_path=FULL_TEXT_FIXTURES / "structured_paper.md",
        )
    )
    return build_page_index(ingestion)


def build_fixture_patch(evidence: EvidencePath) -> ExtractionPatch:
    claim = Claim(
        claim_id="claim:2605.12345:method:chunk-0001:local-markdown-pageindex",
        source_id="2605.12345",
        text="Local markdown is enough to build a deterministic PageIndex.",
        claim_type="method",
        confidence=0.91,
        evidence_path=evidence,
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "fixture"},
    )
    entity = ScientificEntity(
        entity_id="entity:2605.12345:pageindex",
        source_id="2605.12345",
        canonical_name="PageIndex",
        entity_type="method",
        confidence=0.88,
        evidence_path=evidence,
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "fixture"},
    )
    relation = ScientificRelation(
        relation_id="relation:2605.12345:claim-local-markdown-pageindex:entity-pageindex:SUPPORTS",
        source_id="2605.12345",
        relation_type="SUPPORTS",
        from_entity_id=claim.claim_id,
        to_entity_id=entity.entity_id,
        confidence=0.84,
        evidence_path=evidence,
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "fixture"},
    )
    return ExtractionPatch(
        source_id="2605.12345",
        claims=[claim],
        entities=[entity],
        relations=[relation],
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "fixture"},
    )


def build_fixture_payload() -> tuple[
    PageIndexDocument, list[Any], list[EvidencePath], ExtractionPatch
]:
    document = build_document()
    chunks = build_semantic_chunks(document)
    method = next(chunk for chunk in chunks if chunk.page_index_node_id == "2605.12345:method")
    evidence = build_evidence_path(document, method)
    return document, chunks, [evidence], build_fixture_patch(evidence)


def test_init_db_creates_scientific_kg_schema(tmp_path: Path) -> None:
    """Schema initialization exposes the S05 node and relationship labels."""
    conn = init_db(tmp_path / "sci_kg")

    assert _scalar(conn, "MATCH (n:PageIndexNode) RETURN count(n)") == 0
    assert _scalar(conn, "MATCH (n:SemanticChunk) RETURN count(n)") == 0
    assert _scalar(conn, "MATCH (n:EvidencePath) RETURN count(n)") == 0
    assert _scalar(conn, "MATCH (n:Claim) RETURN count(n)") == 0
    assert _scalar(conn, "MATCH (n:ScientificEntity) RETURN count(n)") == 0
    assert _scalar(conn, "MATCH (n:ScientificRelation) RETURN count(n)") == 0


def test_upsert_scientific_kg_persists_fixture_idempotently() -> None:
    """A fixture document plus ExtractionPatch is written once across duplicate reruns."""
    db = ladybug.Database(":memory:")
    conn = ladybug.Connection(db)
    ladybug_client.init_scientific_kg_schema(conn)
    document, chunks, evidence_paths, patch = build_fixture_payload()

    ladybug_client.upsert_scientific_kg(conn, document, chunks, evidence_paths, patch)
    ladybug_client.upsert_scientific_kg(conn, document, chunks, evidence_paths, patch)

    assert _scalar(conn, "MATCH (p:Paper {id: '2605.12345'}) RETURN count(p)") == 1
    assert _scalar(conn, "MATCH (n:PageIndexNode) RETURN count(n)") == len(document.nodes)
    assert _scalar(conn, "MATCH (c:SemanticChunk) RETURN count(c)") == len(chunks)
    assert _scalar(conn, "MATCH (e:EvidencePath) RETURN count(e)") == 1
    assert _scalar(conn, "MATCH (c:Claim) RETURN count(c)") == 1
    assert _scalar(conn, "MATCH (e:ScientificEntity) RETURN count(e)") == 1
    assert _scalar(conn, "MATCH (r:ScientificRelation) RETURN count(r)") == 1
    assert _rows(
        conn.execute(
            "MATCH (claim:Claim)-[:EVIDENCED_BY]->(evidence:EvidencePath) "
            "RETURN claim.id, evidence.id"
        )
    ) == [[patch.claims[0].claim_id, "evidence:2605.12345:method:2605.12345:method:chunk-0001"]]


class FailingIfTransactionConn:
    def execute(self, query: str, params: dict[str, Any] | None = None) -> None:
        if query.startswith("BEGIN"):
            raise AssertionError("invalid payload should be rejected before transaction")


def test_upsert_scientific_kg_rejects_invalid_patch_before_transaction() -> None:
    """Invalid S04 drafts should not open a write transaction or create partial state."""
    document, chunks, evidence_paths, patch = build_fixture_payload()
    invalid_claim = replace(patch.claims[0], confidence=1.5)
    invalid_patch = ExtractionPatch(
        source_id=patch.source_id,
        claims=[invalid_claim],
        entities=patch.entities,
        relations=patch.relations,
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "fixture"},
    )

    with pytest.raises(ValueError, match="confidence 1.5 is outside"):
        ladybug_client.upsert_scientific_kg(
            cast(ladybug.Connection, FailingIfTransactionConn()),
            document,
            chunks,
            evidence_paths,
            invalid_patch,
        )


def test_upsert_scientific_kg_rejects_patch_evidence_not_in_persisted_paths_before_transaction() -> None:
    """Draft evidence must be present in the persisted EvidencePath list."""
    document, chunks, _, patch = build_fixture_payload()

    with pytest.raises(ValueError, match="is not included in persisted evidence_paths"):
        ladybug_client.upsert_scientific_kg(
            cast(ladybug.Connection, FailingIfTransactionConn()),
            document,
            chunks,
            [],
            patch,
        )


def test_upsert_scientific_kg_rolls_back_on_mid_write_failure() -> None:
    """A storage failure after BEGIN must issue ROLLBACK and re-raise the error."""
    document, chunks, evidence_paths, patch = build_fixture_payload()

    class FailingConn:
        def __init__(self) -> None:
            self.queries: list[str] = []

        def execute(self, query: str, params: dict[str, Any] | None = None) -> None:
            self.queries.append(query)
            if "MERGE (chunk:SemanticChunk" in query:
                raise RuntimeError("simulated chunk write failure")

    conn = FailingConn()

    with pytest.raises(RuntimeError, match="simulated chunk write failure"):
        ladybug_client.upsert_scientific_kg(
            cast(ladybug.Connection, conn),
            document,
            chunks,
            evidence_paths,
            patch,
        )

    assert conn.queries[0] == "BEGIN TRANSACTION"
    assert conn.queries[-1] == "ROLLBACK"
    assert "COMMIT" not in conn.queries
