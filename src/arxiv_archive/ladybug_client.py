from __future__ import annotations

import logging
from pathlib import Path
from typing import TYPE_CHECKING

import ladybug

from arxiv_archive.evidence import EvidencePath, SemanticChunk
from research_graph.papers.indexing import PageIndexDocument
from arxiv_archive.scientific_extraction import (
    ExtractionPatch,
    ScientificRelation,
    validate_extraction_patch,
)

if TYPE_CHECKING:
    from arxiv_archive.cli import DailyAnalysis

logger = logging.getLogger(__name__)

DB_DIR = Path.home() / ".research" / "graph_db"

_BASE_SCHEMA_STATEMENTS = [
    "CREATE NODE TABLE Paper(id STRING, title STRING, published DATE, emb FLOAT[512], score DOUBLE, PRIMARY KEY (id))",
    "CREATE NODE TABLE Author(name STRING, PRIMARY KEY (name))",
    "CREATE NODE TABLE Keyword(word STRING, PRIMARY KEY (word))",
    "CREATE NODE TABLE Category(name STRING, PRIMARY KEY (name))",
    "CREATE REL TABLE AUTHORED_BY(FROM Paper TO Author)",
    "CREATE REL TABLE TAGGED_WITH(FROM Paper TO Keyword)",
    "CREATE REL TABLE BELONGS_TO(FROM Paper TO Category)",
]

_SCIENTIFIC_KG_SCHEMA_STATEMENTS = [
    "CREATE NODE TABLE PageIndexNode("
    "id STRING, paper_id STRING, title STRING, level INT64, node_order INT64, text STRING, "
    "source_path STRING, parent_id STRING, next_id STRING, path STRING, PRIMARY KEY (id))",
    "CREATE NODE TABLE SemanticChunk("
    "id STRING, paper_id STRING, page_index_node_id STRING, chunk_order INT64, text STRING, "
    "char_start INT64, char_end INT64, chunking_strategy STRING, path STRING, PRIMARY KEY (id))",
    "CREATE NODE TABLE EvidencePath("
    "id STRING, paper_id STRING, page_index_node_id STRING, semantic_chunk_id STRING, node_path STRING, "
    "PRIMARY KEY (id))",
    "CREATE NODE TABLE Claim("
    "id STRING, paper_id STRING, text STRING, claim_type STRING, confidence DOUBLE, schema_version STRING, "
    "extractor_version STRING, PRIMARY KEY (id))",
    "CREATE NODE TABLE ScientificEntity("
    "id STRING, paper_id STRING, label STRING, entity_type STRING, confidence DOUBLE, schema_version STRING, "
    "extractor_version STRING, PRIMARY KEY (id))",
    "CREATE NODE TABLE ScientificRelation("
    "id STRING, paper_id STRING, relation_type STRING, source_id STRING, target_id STRING, confidence DOUBLE, "
    "schema_version STRING, extractor_version STRING, PRIMARY KEY (id))",
    "CREATE REL TABLE HAS_PAGE_INDEX_NODE(FROM Paper TO PageIndexNode)",
    "CREATE REL TABLE CHILD_PAGE_INDEX_NODE(FROM PageIndexNode TO PageIndexNode)",
    "CREATE REL TABLE NEXT_PAGE_INDEX_NODE(FROM PageIndexNode TO PageIndexNode)",
    "CREATE REL TABLE HAS_SEMANTIC_CHUNK(FROM PageIndexNode TO SemanticChunk)",
    "CREATE REL TABLE EVIDENCE_PAGE_INDEX_NODE(FROM EvidencePath TO PageIndexNode)",
    "CREATE REL TABLE EVIDENCE_SEMANTIC_CHUNK(FROM EvidencePath TO SemanticChunk)",
    "CREATE REL TABLE EVIDENCED_BY("
    "FROM Claim TO EvidencePath, FROM ScientificEntity TO EvidencePath, FROM ScientificRelation TO EvidencePath)",
    "CREATE REL TABLE SCIENTIFIC_RELATION_SOURCE("
    "FROM ScientificRelation TO Claim, FROM ScientificRelation TO ScientificEntity)",
    "CREATE REL TABLE SCIENTIFIC_RELATION_TARGET("
    "FROM ScientificRelation TO Claim, FROM ScientificRelation TO ScientificEntity)",
    "CREATE REL TABLE SCIENTIFIC_RELATION("
    "FROM Claim TO Claim, FROM Claim TO ScientificEntity, FROM ScientificEntity TO Claim, "
    "FROM ScientificEntity TO ScientificEntity, relation_id STRING, relation_type STRING, confidence DOUBLE)",
]


def init_db(db_path: Path | str = DB_DIR) -> ladybug.Connection:
    """Initialize LadybugDB and ensure the graph schema exists.

    Creates the M002 archive graph schema plus the M003 S05 scientific KG
    fixture schema. Extension loading remains best-effort; schema errors other
    than already-existing tables are surfaced to callers.
    """
    path_str = str(db_path)
    Path(path_str).parent.mkdir(parents=True, exist_ok=True)

    db = ladybug.Database(path_str)
    conn = ladybug.Connection(db)

    try:
        conn.execute("INSTALL algo;")
        conn.execute("LOAD EXTENSION algo;")
    except Exception as e:
        logger.warning(f"Could not load algo extension: {e}")

    init_base_schema(conn)
    init_scientific_kg_schema(conn)
    return conn


def init_base_schema(conn: ladybug.Connection) -> None:
    """Create the original daily archive schema idempotently."""
    _execute_schema_statements(conn, _BASE_SCHEMA_STATEMENTS, schema_name="base archive")


def init_scientific_kg_schema(conn: ladybug.Connection) -> None:
    """Create the M003 scientific KG fixture schema idempotently."""
    init_base_schema(conn)
    _execute_schema_statements(conn, _SCIENTIFIC_KG_SCHEMA_STATEMENTS, schema_name="scientific KG")


def upsert_daily_analysis(conn: ladybug.Connection, analysis: DailyAnalysis) -> None:
    """Bulk upsert a DailyAnalysis payload into LadybugDB.

    Uses explicit transactions and parameterized MERGE statements to handle deduplication
    gracefully and ensure atomic single-writer concurrency.
    """
    if analysis.status == "empty" or not analysis.papers:
        return

    conn.execute("BEGIN TRANSACTION")
    try:
        for p in analysis.papers:
            paper = p.paper

            # 1. Upsert Paper
            emb_list = p.embedding if p.embedding else []
            conn.execute(
                "MERGE (p:Paper {id: $id}) "
                "ON MATCH SET p.title = $title, p.published = date($published), p.emb = $emb, p.score = $score "
                "ON CREATE SET p.title = $title, p.published = date($published), p.emb = $emb, p.score = $score",
                {
                    "id": paper.id,
                    "title": paper.title,
                    "published": paper.published.isoformat(),
                    "emb": emb_list,
                    "score": p.score,
                },
            )

            # 2. Upsert Authors and AUTHORED_BY
            for author in paper.authors:
                conn.execute("MERGE (a:Author {name: $name})", {"name": author})
                conn.execute(
                    "MATCH (p:Paper {id: $id}), (a:Author {name: $name}) "
                    "MERGE (p)-[:AUTHORED_BY]->(a)",
                    {"id": paper.id, "name": author},
                )

            # 3. Upsert Categories and BELONGS_TO
            for cat in paper.categories:
                conn.execute("MERGE (c:Category {name: $name})", {"name": cat})
                conn.execute(
                    "MATCH (p:Paper {id: $id}), (c:Category {name: $name}) "
                    "MERGE (p)-[:BELONGS_TO]->(c)",
                    {"id": paper.id, "name": cat},
                )

            # 4. Upsert Keywords and TAGGED_WITH
            for keyword in p.keywords:
                conn.execute("MERGE (k:Keyword {word: $word})", {"word": keyword})
                conn.execute(
                    "MATCH (p:Paper {id: $id}), (k:Keyword {word: $word}) "
                    "MERGE (p)-[:TAGGED_WITH]->(k)",
                    {"id": paper.id, "name": keyword, "word": keyword},
                )

        conn.execute("COMMIT")
        logger.info(f"Bulk upserted {len(analysis.papers)} papers into LadybugDB.")
    except Exception as e:
        conn.execute("ROLLBACK")
        logger.error(f"Failed to bulk upsert papers: {e}")
        raise


def upsert_scientific_kg(
    conn: ladybug.Connection,
    document: PageIndexDocument,
    chunks: list[SemanticChunk],
    evidence_paths: list[EvidencePath],
    patch: ExtractionPatch,
) -> None:
    """Persist one fixture scientific KG patch transactionally and idempotently."""
    _validate_scientific_kg_payload(document, chunks, evidence_paths, patch)

    conn.execute("BEGIN TRANSACTION")
    try:
        _merge_scientific_paper(conn, document)
        _merge_page_index_nodes(conn, document)
        _merge_semantic_chunks(conn, chunks)
        _merge_evidence_paths(conn, evidence_paths)
        _merge_extraction_patch(conn, patch)
        conn.execute("COMMIT")
        logger.info(
            "Upserted scientific KG paper_id=%s nodes=%d chunks=%d evidence_paths=%d claims=%d entities=%d relations=%d",
            document.paper_id,
            len(document.nodes),
            len(chunks),
            len(evidence_paths),
            len(patch.claims),
            len(patch.entities),
            len(patch.relations),
        )
    except Exception as e:
        conn.execute("ROLLBACK")
        logger.error(
            "Failed to upsert scientific KG paper_id=%s phase=write error=%s", document.paper_id, e
        )
        raise


def evidence_path_id(path: EvidencePath) -> str:
    """Return the deterministic LadybugDB ID for an EvidencePath node."""
    return f"evidence:{path.page_index_node_id}:{path.semantic_chunk_id}"


def _execute_schema_statements(
    conn: ladybug.Connection, statements: list[str], *, schema_name: str
) -> None:
    created = 0
    existing = 0
    for statement in statements:
        try:
            conn.execute(statement)
            created += 1
        except RuntimeError as e:
            if "already exists" not in str(e).lower():
                raise
            existing += 1
    logger.info("LadybugDB %s schema ready created=%d existing=%d", schema_name, created, existing)


def _validate_scientific_kg_payload(
    document: PageIndexDocument,
    chunks: list[SemanticChunk],
    evidence_paths: list[EvidencePath],
    patch: ExtractionPatch,
) -> None:
    diagnostics: list[str] = []
    if patch.paper_id != document.paper_id:
        diagnostics.append(
            f"ExtractionPatch {patch.paper_id} does not match document paper_id {document.paper_id}"
        )

    chunk_ids = {chunk.id for chunk in chunks}
    node_ids = {node.id for node in document.nodes}
    for chunk in chunks:
        if chunk.paper_id != document.paper_id:
            diagnostics.append(
                f"SemanticChunk {chunk.id} paper_id {chunk.paper_id} does not match document paper_id {document.paper_id}"
            )
        if chunk.page_index_node_id not in node_ids:
            diagnostics.append(
                f"SemanticChunk {chunk.id} references missing PageIndexNode {chunk.page_index_node_id}"
            )

    persisted_evidence_ids: set[str] = set()
    for path in evidence_paths:
        path_id = evidence_path_id(path)
        persisted_evidence_ids.add(path_id)
        if path.validation_warnings:
            diagnostics.append(
                f"EvidencePath {path_id} has validation warnings: "
                + "; ".join(path.validation_warnings)
            )
        if path.paper_id != document.paper_id:
            diagnostics.append(
                f"EvidencePath {path_id} paper_id {path.paper_id} does not match document paper_id {document.paper_id}"
            )
        if path.page_index_node_id not in node_ids:
            diagnostics.append(
                f"EvidencePath {path_id} references missing PageIndexNode {path.page_index_node_id}"
            )
        if path.semantic_chunk_id not in chunk_ids:
            diagnostics.append(
                f"EvidencePath {path_id} references missing SemanticChunk {path.semantic_chunk_id}"
            )

    for kind, item in [
        *(("Claim", claim) for claim in patch.claims),
        *(("ScientificEntity", entity) for entity in patch.entities),
        *(("ScientificRelation", relation) for relation in patch.relations),
    ]:
        if item.evidence_path is None:
            continue
        item_evidence_id = evidence_path_id(item.evidence_path)
        if item_evidence_id not in persisted_evidence_ids:
            diagnostics.append(
                f"{kind} {item.id} evidence_path {item_evidence_id} is not included in persisted evidence_paths"
            )

    diagnostics.extend(validate_extraction_patch(patch))
    if diagnostics:
        raise ValueError("Invalid scientific KG payload: " + "; ".join(diagnostics))


def _merge_scientific_paper(conn: ladybug.Connection, document: PageIndexDocument) -> None:
    conn.execute("MERGE (paper:Paper {id: $id})", {"id": document.paper_id})


def _merge_page_index_nodes(conn: ladybug.Connection, document: PageIndexDocument) -> None:
    for node in document.nodes:
        conn.execute(
            "MERGE (node:PageIndexNode {id: $id}) "
            "ON MATCH SET node.paper_id = $paper_id, node.title = $title, node.level = $level, "
            "node.node_order = $node_order, node.text = $text, node.source_path = $source_path, "
            "node.parent_id = $parent_id, node.next_id = $next_id, node.path = $path "
            "ON CREATE SET node.paper_id = $paper_id, node.title = $title, node.level = $level, "
            "node.node_order = $node_order, node.text = $text, node.source_path = $source_path, "
            "node.parent_id = $parent_id, node.next_id = $next_id, node.path = $path",
            {
                "id": node.id,
                "paper_id": node.paper_id,
                "title": node.title,
                "level": node.level,
                "node_order": node.order,
                "text": node.text,
                "source_path": str(node.source_path),
                "parent_id": node.parent_id,
                "next_id": node.next_id,
                "path": "/".join(node.path),
            },
        )
        conn.execute(
            "MATCH (paper:Paper {id: $paper_id}), (node:PageIndexNode {id: $node_id}) "
            "MERGE (paper)-[:HAS_PAGE_INDEX_NODE]->(node)",
            {"paper_id": document.paper_id, "node_id": node.id},
        )
        if node.parent_id is not None:
            conn.execute(
                "MATCH (parent:PageIndexNode {id: $parent_id}), (child:PageIndexNode {id: $child_id}) "
                "MERGE (parent)-[:CHILD_PAGE_INDEX_NODE]->(child)",
                {"parent_id": node.parent_id, "child_id": node.id},
            )
        if node.next_id is not None:
            conn.execute(
                "MATCH (current:PageIndexNode {id: $current_id}), (next:PageIndexNode {id: $next_id}) "
                "MERGE (current)-[:NEXT_PAGE_INDEX_NODE]->(next)",
                {"current_id": node.id, "next_id": node.next_id},
            )


def _merge_semantic_chunks(conn: ladybug.Connection, chunks: list[SemanticChunk]) -> None:
    for chunk in chunks:
        conn.execute(
            "MERGE (chunk:SemanticChunk {id: $id}) "
            "ON MATCH SET chunk.paper_id = $paper_id, chunk.page_index_node_id = $page_index_node_id, "
            "chunk.chunk_order = $chunk_order, chunk.text = $text, chunk.char_start = $char_start, "
            "chunk.char_end = $char_end, chunk.chunking_strategy = $chunking_strategy, chunk.path = $path "
            "ON CREATE SET chunk.paper_id = $paper_id, chunk.page_index_node_id = $page_index_node_id, "
            "chunk.chunk_order = $chunk_order, chunk.text = $text, chunk.char_start = $char_start, "
            "chunk.char_end = $char_end, chunk.chunking_strategy = $chunking_strategy, chunk.path = $path",
            {
                "id": chunk.id,
                "paper_id": chunk.paper_id,
                "page_index_node_id": chunk.page_index_node_id,
                "chunk_order": chunk.order,
                "text": chunk.text,
                "char_start": chunk.char_start,
                "char_end": chunk.char_end,
                "chunking_strategy": chunk.chunking_strategy,
                "path": "/".join(chunk.page_index_path),
            },
        )
        conn.execute(
            "MATCH (node:PageIndexNode {id: $node_id}), (chunk:SemanticChunk {id: $chunk_id}) "
            "MERGE (node)-[:HAS_SEMANTIC_CHUNK]->(chunk)",
            {"node_id": chunk.page_index_node_id, "chunk_id": chunk.id},
        )


def _merge_evidence_paths(conn: ladybug.Connection, evidence_paths: list[EvidencePath]) -> None:
    for path in evidence_paths:
        path_id = evidence_path_id(path)
        conn.execute(
            "MERGE (evidence:EvidencePath {id: $id}) "
            "ON MATCH SET evidence.paper_id = $paper_id, evidence.page_index_node_id = $page_index_node_id, "
            "evidence.semantic_chunk_id = $semantic_chunk_id, evidence.node_path = $node_path "
            "ON CREATE SET evidence.paper_id = $paper_id, evidence.page_index_node_id = $page_index_node_id, "
            "evidence.semantic_chunk_id = $semantic_chunk_id, evidence.node_path = $node_path",
            {
                "id": path_id,
                "paper_id": path.paper_id,
                "page_index_node_id": path.page_index_node_id,
                "semantic_chunk_id": path.semantic_chunk_id,
                "node_path": "/".join(path.node_path),
            },
        )
        conn.execute(
            "MATCH (evidence:EvidencePath {id: $evidence_id}), (node:PageIndexNode {id: $node_id}) "
            "MERGE (evidence)-[:EVIDENCE_PAGE_INDEX_NODE]->(node)",
            {"evidence_id": path_id, "node_id": path.page_index_node_id},
        )
        conn.execute(
            "MATCH (evidence:EvidencePath {id: $evidence_id}), (chunk:SemanticChunk {id: $chunk_id}) "
            "MERGE (evidence)-[:EVIDENCE_SEMANTIC_CHUNK]->(chunk)",
            {"evidence_id": path_id, "chunk_id": path.semantic_chunk_id},
        )


def _merge_extraction_patch(conn: ladybug.Connection, patch: ExtractionPatch) -> None:
    evidence_by_chunk = {
        item.evidence_path.semantic_chunk_id: evidence_path_id(item.evidence_path)
        for item in [*patch.claims, *patch.entities, *patch.relations]
        if item.evidence_path is not None
    }

    for claim in patch.claims:
        conn.execute(
            "MERGE (claim:Claim {id: $id}) "
            "ON MATCH SET claim.paper_id = $paper_id, claim.text = $text, claim.claim_type = $claim_type, "
            "claim.confidence = $confidence, claim.schema_version = $schema_version, "
            "claim.extractor_version = $extractor_version "
            "ON CREATE SET claim.paper_id = $paper_id, claim.text = $text, claim.claim_type = $claim_type, "
            "claim.confidence = $confidence, claim.schema_version = $schema_version, "
            "claim.extractor_version = $extractor_version",
            {
                "id": claim.id,
                "paper_id": claim.paper_id,
                "text": claim.text,
                "claim_type": claim.claim_type,
                "confidence": claim.confidence,
                "schema_version": claim.schema_version,
                "extractor_version": claim.extractor_version,
            },
        )
        if claim.evidence_path is None:
            raise ValueError(f"Claim {claim.id} is missing evidence_path")
        _merge_evidenced_by(
            conn,
            "Claim",
            "claim",
            claim.id,
            evidence_by_chunk[claim.evidence_path.semantic_chunk_id],
        )

    for entity in patch.entities:
        conn.execute(
            "MERGE (entity:ScientificEntity {id: $id}) "
            "ON MATCH SET entity.paper_id = $paper_id, entity.label = $label, entity.entity_type = $entity_type, "
            "entity.confidence = $confidence, entity.schema_version = $schema_version, "
            "entity.extractor_version = $extractor_version "
            "ON CREATE SET entity.paper_id = $paper_id, entity.label = $label, entity.entity_type = $entity_type, "
            "entity.confidence = $confidence, entity.schema_version = $schema_version, "
            "entity.extractor_version = $extractor_version",
            {
                "id": entity.id,
                "paper_id": entity.paper_id,
                "label": entity.label,
                "entity_type": entity.entity_type,
                "confidence": entity.confidence,
                "schema_version": entity.schema_version,
                "extractor_version": entity.extractor_version,
            },
        )
        if entity.evidence_path is None:
            raise ValueError(f"ScientificEntity {entity.id} is missing evidence_path")
        _merge_evidenced_by(
            conn,
            "ScientificEntity",
            "entity",
            entity.id,
            evidence_by_chunk[entity.evidence_path.semantic_chunk_id],
        )

    endpoint_labels = {item.id: "Claim" for item in patch.claims}
    endpoint_labels.update({item.id: "ScientificEntity" for item in patch.entities})
    for relation in patch.relations:
        conn.execute(
            "MERGE (relation:ScientificRelation {id: $id}) "
            "ON MATCH SET relation.paper_id = $paper_id, relation.relation_type = $relation_type, "
            "relation.source_id = $source_id, relation.target_id = $target_id, relation.confidence = $confidence, "
            "relation.schema_version = $schema_version, relation.extractor_version = $extractor_version "
            "ON CREATE SET relation.paper_id = $paper_id, relation.relation_type = $relation_type, "
            "relation.source_id = $source_id, relation.target_id = $target_id, relation.confidence = $confidence, "
            "relation.schema_version = $schema_version, relation.extractor_version = $extractor_version",
            {
                "id": relation.id,
                "paper_id": relation.paper_id,
                "relation_type": relation.relation_type,
                "source_id": relation.source_id,
                "target_id": relation.target_id,
                "confidence": relation.confidence,
                "schema_version": relation.schema_version,
                "extractor_version": relation.extractor_version,
            },
        )
        if relation.evidence_path is None:
            raise ValueError(f"ScientificRelation {relation.id} is missing evidence_path")
        _merge_evidenced_by(
            conn,
            "ScientificRelation",
            "relation",
            relation.id,
            evidence_by_chunk[relation.evidence_path.semantic_chunk_id],
        )
        _merge_relation_endpoint(
            conn,
            rel_table="SCIENTIFIC_RELATION_SOURCE",
            endpoint_role="source",
            endpoint_label=endpoint_labels[relation.source_id],
            relation_id=relation.id,
            endpoint_id=relation.source_id,
        )
        _merge_relation_endpoint(
            conn,
            rel_table="SCIENTIFIC_RELATION_TARGET",
            endpoint_role="target",
            endpoint_label=endpoint_labels[relation.target_id],
            relation_id=relation.id,
            endpoint_id=relation.target_id,
        )
        _merge_scientific_relation_edge(conn, relation, endpoint_labels)


def _merge_evidenced_by(
    conn: ladybug.Connection,
    draft_label: str,
    draft_alias: str,
    draft_id: str,
    evidence_id: str,
) -> None:
    conn.execute(
        f"MATCH ({draft_alias}:{draft_label} {{id: $draft_id}}), (evidence:EvidencePath {{id: $evidence_id}}) "
        f"MERGE ({draft_alias})-[:EVIDENCED_BY]->(evidence)",
        {"draft_id": draft_id, "evidence_id": evidence_id},
    )


def _merge_relation_endpoint(
    conn: ladybug.Connection,
    *,
    rel_table: str,
    endpoint_role: str,
    endpoint_label: str,
    relation_id: str,
    endpoint_id: str,
) -> None:
    conn.execute(
        f"MATCH (relation:ScientificRelation {{id: $relation_id}}), ({endpoint_role}:{endpoint_label} {{id: $endpoint_id}}) "
        f"MERGE (relation)-[:{rel_table}]->({endpoint_role})",
        {"relation_id": relation_id, "endpoint_id": endpoint_id},
    )


def _merge_scientific_relation_edge(
    conn: ladybug.Connection,
    relation: ScientificRelation,
    endpoint_labels: dict[str, str],
) -> None:
    source_label = endpoint_labels[relation.source_id]
    target_label = endpoint_labels[relation.target_id]
    conn.execute(
        f"MATCH (source:{source_label} {{id: $source_id}}), (target:{target_label} {{id: $target_id}}) "
        "MERGE (source)-[edge:SCIENTIFIC_RELATION {relation_id: $relation_id}]->(target) "
        "ON MATCH SET edge.relation_type = $relation_type, edge.confidence = $confidence "
        "ON CREATE SET edge.relation_type = $relation_type, edge.confidence = $confidence",
        {
            "source_id": relation.source_id,
            "target_id": relation.target_id,
            "relation_id": relation.id,
            "relation_type": relation.relation_type,
            "confidence": relation.confidence,
        },
    )


__all__ = [
    "evidence_path_id",
    "init_base_schema",
    "init_db",
    "init_scientific_kg_schema",
    "upsert_daily_analysis",
    "upsert_scientific_kg",
]
