"""PageIndex construction from parser output.

This module owns indexing only: it converts typed parser elements into the
legacy-compatible PageIndex tree and navigation summaries. Source loading and
markdown parsing live outside this module.


Formerly: src/arxiv_archive/indexing/page_index.py"""

from __future__ import annotations

from research_graph.corpus.ingestion import FullTextIngestionResult
from research_graph.corpus.parsing.parser import parse_article
from research_graph.corpus.parsing.structure import ParsedArticle, ParsedArticleElement
from research_graph.domain.navigation import (
    PageIndexDocument,
    PageIndexNode,
    build_navigation_anchors,
)


def build_page_index(ingestion: FullTextIngestionResult) -> PageIndexDocument:
    """Build a deterministic PageIndex tree from local full-text ingestion output."""
    return build_page_index_from_parsed(parse_article(ingestion))


def build_page_index_from_parsed(article: ParsedArticle) -> PageIndexDocument:
    """Build a deterministic PageIndex tree from parser output."""
    nodes = [_node_from_element(element) for element in article.elements]
    by_id = {node.id: node for node in nodes}

    for node in nodes:
        if node.parent_id is None:
            continue
        parent = by_id.get(node.parent_id)
        if parent is not None and node.id not in parent.children_ids:
            parent.children_ids.append(node.id)

    for current, next_node in zip(nodes, nodes[1:], strict=False):
        current.next_id = next_node.id

    provenance = dict(article.provenance)
    provenance["page_index_builder"] = "parsed_article_v1"
    provenance["node_count"] = str(len(nodes))
    provenance["navigation_anchor_count"] = str(len(nodes))
    return PageIndexDocument(
        paper_id=article.paper_id,
        source_path=article.source_path,
        root=nodes[0],
        nodes=nodes,
        validation_warnings=list(article.validation_warnings),
        provenance=provenance,
        navigation_anchors=build_navigation_anchors(nodes),
    )


def _node_from_element(element: ParsedArticleElement) -> PageIndexNode:
    return PageIndexNode(
        id=element.id,
        paper_id=element.paper_id,
        title=element.title,
        level=element.level,
        order=element.order,
        text=element.text,
        source_path=element.source_path,
        parent_id=element.parent_id,
        path=list(element.path),
        provenance=dict(element.provenance),
    )


__all__ = [
    "PageIndexDocument",
    "PageIndexNode",
    "build_page_index",
    "build_page_index_from_parsed",
]
