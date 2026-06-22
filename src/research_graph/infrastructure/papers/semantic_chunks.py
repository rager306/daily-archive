# Formerly: src/arxiv_archive/evidence.py (renamed to semantic_chunks for clarity)

"""Semantic chunk and evidence-path contracts for scientific KG fixtures.

This module is deterministic and local-only. It consumes S02 PageIndexDocument
objects and emits section-level SemanticChunk records plus EvidencePath
references that later claim/entity/relation and LadybugDB layers can validate.
"""

from __future__ import annotations

from research_graph.domain.navigation import PageIndexDocument
from research_graph.domain.semantic_chunks import EvidencePath, SemanticChunk
from research_graph.infrastructure.corpus.parsing.structure import ParsedArticle
from research_graph.infrastructure.papers.indexing.parsed_page_index import (
    build_page_index_from_parsed,
)

DEFAULT_CHUNKING_STRATEGY = "section_text_v1"

# Canonical model home is research_graph.domain.semantic_chunks (D086).
# EvidencePath / SemanticChunk are re-exported below for back-compat.


def build_semantic_chunks_from_parsed(
    article: ParsedArticle,
    *,
    chunking_strategy: str = DEFAULT_CHUNKING_STRATEGY,
) -> list[SemanticChunk]:
    """Build deterministic chunks from parser output via the canonical PageIndex builder."""
    return build_semantic_chunks(
        build_page_index_from_parsed(article),
        chunking_strategy=chunking_strategy,
    )


def build_semantic_chunks(
    document: PageIndexDocument,
    *,
    chunking_strategy: str = DEFAULT_CHUNKING_STRATEGY,
) -> list[SemanticChunk]:
    """Build deterministic section-level chunks for non-empty PageIndex nodes."""
    chunks: list[SemanticChunk] = []
    document.validation_warnings[:] = [
        warning
        for warning in document.validation_warnings
        if "has empty text; no SemanticChunk emitted" not in warning
    ]

    for node in document.nodes:
        if node.id == document.root.id:
            continue

        text = node.text.strip()
        if not text:
            document.validation_warnings.append(
                f"PageIndexNode {node.id} has empty text; no SemanticChunk emitted"
            )
            continue

        chunk_id = f"{node.id}:chunk-0001"
        chunks.append(
            SemanticChunk(
                id=chunk_id,
                paper_id=document.paper_id,
                page_index_node_id=node.id,
                page_index_path=list(node.path),
                order=len(chunks),
                text=text,
                char_start=0,
                char_end=len(text),
                chunking_strategy=chunking_strategy,
                validation_warnings=[],
                provenance={
                    "paper_id": document.paper_id,
                    "page_index_node_id": node.id,
                    "page_index_path": "/".join(node.path),
                    "chunking_strategy": chunking_strategy,
                    "source_path": str(document.source_path),
                },
            )
        )

    return chunks


def build_evidence_paths(
    document: PageIndexDocument, chunks: list[SemanticChunk]
) -> list[EvidencePath]:
    """Build and validate EvidencePath records for a document's semantic chunks."""
    return [build_evidence_path(document, chunk) for chunk in chunks]


def build_evidence_path(document: PageIndexDocument, chunk: SemanticChunk) -> EvidencePath:
    """Build and validate an EvidencePath for one chunk."""
    path = EvidencePath(
        paper_id=document.paper_id,
        page_index_node_id=chunk.page_index_node_id,
        semantic_chunk_id=chunk.id,
        node_path=list(chunk.page_index_path),
        validation_warnings=[],
        provenance={
            "paper_id": document.paper_id,
            "page_index_node_id": chunk.page_index_node_id,
            "semantic_chunk_id": chunk.id,
            "source_path": str(document.source_path),
        },
    )
    warnings = validate_evidence_path(path, document, [chunk])
    if not warnings:
        return path
    return EvidencePath(
        paper_id=path.paper_id,
        page_index_node_id=path.page_index_node_id,
        semantic_chunk_id=path.semantic_chunk_id,
        node_path=path.node_path,
        validation_warnings=warnings,
        provenance=path.provenance,
    )


def validate_evidence_path(
    path: EvidencePath,
    document: PageIndexDocument,
    chunks: list[SemanticChunk],
) -> list[str]:
    """Return diagnostics for broken Paper -> PageIndexNode -> SemanticChunk links."""
    diagnostics: list[str] = []
    chunks_by_id = {chunk.id: chunk for chunk in chunks}

    if path.paper_id != document.paper_id:
        diagnostics.append(
            f"evidence path paper_id {path.paper_id} does not match document paper_id {document.paper_id}"
        )

    node = document.node_by_id(path.page_index_node_id)
    if node is None:
        diagnostics.append(
            f"evidence path references missing PageIndexNode {path.page_index_node_id}"
        )

    chunk = chunks_by_id.get(path.semantic_chunk_id)
    if chunk is None:
        diagnostics.append(
            f"evidence path references missing SemanticChunk {path.semantic_chunk_id}"
        )
    elif chunk.page_index_node_id != path.page_index_node_id:
        diagnostics.append(
            f"SemanticChunk {chunk.id} belongs to node {chunk.page_index_node_id}, "
            f"not {path.page_index_node_id}"
        )

    if chunk is not None and chunk.paper_id != path.paper_id:
        diagnostics.append(
            f"SemanticChunk {chunk.id} paper_id {chunk.paper_id} does not match evidence path "
            f"paper_id {path.paper_id}"
        )

    if node is not None and list(path.node_path) != list(node.path):
        diagnostics.append(
            f"evidence path node_path {'/'.join(path.node_path)} does not match PageIndexNode "
            f"path {'/'.join(node.path)}"
        )

    return diagnostics


__all__ = [
    "EvidencePath",
    "SemanticChunk",
    "build_evidence_path",
    "build_evidence_paths",
    "build_semantic_chunks",
    "build_semantic_chunks_from_parsed",
    "validate_evidence_path",
]
