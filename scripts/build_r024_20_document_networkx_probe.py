#!/usr/bin/env python3
"""R024 S04: extended NetworkX graph probe on 20 articles.

Builds extended DiGraph from 20-article corpus:
- Nodes: 1 corpus + 20 articles + 40 chunks + 100 entities (5 types x 20 articles)
- Edges: corpus→article, article→chunk, article→entity, article_cites_article (topic_tags-based)

Entity types derived from M025 chunk types:
- metadata
- table_context
- figure_caption_context
- citation_context
- retrieval_context

NO LadybugDB. NO FalkorDB. NO Neo4j.

Outputs:
- data/r024-20-document-corpus-v1/networkx-probe/probe.graphml
- data/r024-20-document-corpus-v1/networkx-probe/summary.json
- data/r024-20-document-corpus-v1/networkx-probe/events.jsonl
"""

from __future__ import annotations

import json
import sys
from collections import defaultdict
from datetime import UTC, datetime
from pathlib import Path

REPO_ROOT = Path("/root/daily-archive")
R020_DIR = REPO_ROOT / "data" / "r024-20-document-corpus-v1"
PROBE_DIR = R020_DIR / "networkx-probe"
SELECTION = R020_DIR / "selection.json"
EVENTS_LOG = R020_DIR / "parser-chunking" / "events.jsonl"
GRAPHML = PROBE_DIR / "probe.graphml"
SUMMARY = PROBE_DIR / "summary.json"
PROBE_EVENTS = PROBE_DIR / "events.jsonl"

ENTITY_TYPES = (
    "metadata",
    "table_context",
    "figure_caption_context",
    "citation_context",
    "retrieval_context",
)

try:
    import networkx as nx  # type: ignore[import-unresolved]
except ImportError:
    print("networkx not installed", file=sys.stderr)
    sys.exit(1)


def find_citation_relations(articles: list[dict]) -> list[tuple[str, str, str]]:
    """Find article_cites_article relations via shared coarse category.

    For 20-doc corpus, use coarse_topic_code from article.json (when available)
    or derive from article_ref path (cs-ai, cs-cl, etc.) as fallback.
    """
    relations: list[tuple[str, str, str]] = []
    by_cat: dict[str, list[str]] = defaultdict(list)
    for a in articles:
        ref = a["article_ref"]
        # derive category from ref path: arxiv/cs-ai/... → cs-ai
        parts = ref.split("/")
        if len(parts) >= 2 and parts[0] == "arxiv":
            cat = parts[1]
        elif len(parts) >= 2 and parts[0] == "company_blog":
            cat = parts[1]
        else:
            cat = "other"
        by_cat[cat].append(ref)
    seen: set[tuple[str, str]] = set()
    for cat, refs in sorted(by_cat.items()):
        if len(refs) < 2:
            continue
        for i, r1 in enumerate(refs):
            for r2 in refs[i + 1 :]:
                pair: tuple[str, str] = (r1, r2) if r1 < r2 else (r2, r1)
                if pair in seen:
                    continue
                seen.add(pair)
                relations.append((r1, r2, cat))
    return relations


def main() -> int:
    PROBE_DIR.mkdir(parents=True, exist_ok=True)
    sel = json.loads(SELECTION.read_text())
    articles = sel["articles"]
    print(f"Building extended NetworkX probe for {len(articles)} articles...")

    chunk_counts: dict[str, int] = {}
    for line in EVENTS_LOG.read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        ev = json.loads(line)
        if ev.get("event") == "parser_chunking_complete":
            chunk_counts[ev["article_ref"]] = int(str(ev.get("chunk_count", 0)))

    events: list[dict[str, object]] = []
    graph = nx.DiGraph(name="r024-20-document-corpus-extended-probe")

    graph.add_node(
        "corpus:r024-20-document-corpus-v1",
        node_type="corpus",
        label="R024 20-document corpus v1",
        scale=20,
    )

    for a in articles:
        ref = a["article_ref"]
        key = a["article_key"]
        n_chunks = chunk_counts.get(ref, 0)

        # Article root node
        article_node_id = f"article:{ref}"
        graph.add_node(
            article_node_id,
            node_type="article",
            article_ref=ref,
            article_key=key,
            selection_role=a.get("selection_role", ""),
            source_code=a.get("source_code", ""),
            topic_tags=",".join(a.get("topic_tags", [])),
        )
        graph.add_edge(
            "corpus:r024-20-document-corpus-v1",
            article_node_id,
            edge_type="corpus_contains_article",
        )

        # Chunk nodes
        for i in range(n_chunks):
            chunk_id = f"chunk:{ref}:{i + 1:04d}"
            graph.add_node(
                chunk_id,
                node_type="chunk",
                article_ref=ref,
                chunk_index=i + 1,
            )
            graph.add_edge(
                article_node_id,
                chunk_id,
                edge_type="article_contains_chunk",
            )

        # Entity nodes (5 types per article, derived from M025 chunk types)
        for entity_type in ENTITY_TYPES:
            entity_id = f"entity:{ref}:{entity_type}"
            graph.add_node(
                entity_id,
                node_type="entity",
                entity_type=entity_type,
                article_ref=ref,
                source="m025_chunk_types",
            )
            graph.add_edge(
                article_node_id,
                entity_id,
                edge_type="article_has_entity",
            )

        events.append(
            {
                "event": "article_added",
                "timestamp": datetime.now(UTC).isoformat(),
                "article_ref": ref,
                "chunks_added": n_chunks,
                "entities_added": len(ENTITY_TYPES),
                "network_fetch_attempted": False,
                "production_import_attempted": False,
                "graph_import_allowed": False,
                "ladybugdb_written": False,
            }
        )
        print(f"  + {ref}: {n_chunks} chunks, {len(ENTITY_TYPES)} entities")

    # Cross-document citation relations (article_cites_article)
    citations = find_citation_relations(articles)
    print(f"  citation relations: {len(citations)}")
    for r1, r2, cat in citations:
        graph.add_edge(
            f"article:{r1}",
            f"article:{r2}",
            edge_type="article_cites_article",
            source=f"coarse_category:{cat}",
        )

    nx.write_graphml(graph, str(GRAPHML))

    n_nodes = graph.number_of_nodes()
    n_edges = graph.number_of_edges()
    node_types: dict[str, int] = {}
    for _, data in graph.nodes(data=True):
        if isinstance(data, dict):
            nt = str(data.get("node_type", "unknown"))
        else:
            nt = "unknown"
        node_types[nt] = node_types.get(nt, 0) + 1
    edge_types: dict[str, int] = {}
    for _, _, data in graph.edges(data=True):
        if isinstance(data, dict):
            et = str(data.get("edge_type", "unknown"))
        else:
            et = "unknown"
        edge_types[et] = edge_types.get(et, 0) + 1

    summary = {
        "schema_version": "r024-20-document-networkx-probe-summary.v00.01",
        "generated_at": datetime.now(UTC).isoformat(),
        "corpus_size": len(articles),
        "n_nodes": n_nodes,
        "n_edges": n_edges,
        "node_types": node_types,
        "edge_types": edge_types,
        "citation_relations_count": len(citations),
        "entity_types": list(ENTITY_TYPES),
        "fail_closed_invariants": {
            "network_fetch_attempted": False,
            "production_import_attempted": False,
            "graph_import_allowed": False,
            "ladybugdb_written": False,
            "trusted_kg_import_allowed": False,
            "graph_readiness_claim": False,
            "falkordb_written": False,
            "neo4j_written": False,
            "ladybugdb_connection_attempted": False,
        },
        "implementation": {
            "library": "networkx",
            "graph_type": "DiGraph",
            "in_memory_only": True,
            "no_db_connection": True,
            "no_network_io": True,
        },
    }
    SUMMARY.write_text(json.dumps(summary, indent=2))

    with open(PROBE_EVENTS, "w") as f:
        for e in events:
            f.write(json.dumps(e) + "\n")

    print(f"summary: nodes={n_nodes}, edges={n_edges}")
    print(f"  node_types={node_types}")
    print(f"  edge_types={edge_types}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
