#!/usr/bin/env python3
"""R024 S04: bounded NetworkX graph probe on 10 articles.

Builds a DiGraph from the 10-article corpus:
- Nodes: 1 article root + N chunks per article
- Edges: article_contains_chunk

NO LadybugDB. NO FalkorDB. NO production import.

Outputs:
- data/r024-10-document-corpus-v1/networkx-probe/probe.graphml
- data/r024-10-document-corpus-v1/networkx-probe/summary.json
- data/r024-10-document-corpus-v1/networkx-probe/events.jsonl
"""

from __future__ import annotations

import json
import sys
from datetime import UTC, datetime
from pathlib import Path

REPO_ROOT = Path("/root/daily-archive")
R024_DIR = REPO_ROOT / "data" / "r024-10-document-corpus-v1"
PROBE_DIR = R024_DIR / "networkx-probe"
SELECTION = R024_DIR / "selection.json"
EVENTS_LOG = R024_DIR / "parser-chunking" / "events.jsonl"
GRAPHML = PROBE_DIR / "probe.graphml"
SUMMARY = PROBE_DIR / "summary.json"
PROBE_EVENTS = PROBE_DIR / "events.jsonl"

# networkx import (in-memory, no DB connections)
try:
    import networkx as nx  # type: ignore[import-unresolved]
except ImportError:
    print("networkx not installed; skipping S04 probe", file=sys.stderr)
    sys.exit(1)


def main() -> int:
    PROBE_DIR.mkdir(parents=True, exist_ok=True)
    sel = json.loads(SELECTION.read_text())
    articles = sel["articles"]
    print(f"Building NetworkX probe for {len(articles)} articles...")

    # Load per-article chunk counts from S02 events.jsonl
    chunk_counts: dict[str, int] = {}
    for line in EVENTS_LOG.read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        ev = json.loads(line)
        if ev.get("event") == "parser_chunking_complete":
            chunk_counts[ev["article_ref"]] = ev.get("chunk_count", 0)

    events: list[dict[str, object]] = []
    graph = nx.DiGraph(name="r024-10-document-corpus-probe")

    # Add source provenance node
    graph.add_node(
        "corpus:r024-10-document-corpus-v1",
        node_type="corpus",
        label="R024 10-document corpus v1",
        scale=10,
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
        )
        # Edge: corpus → article
        graph.add_edge(
            "corpus:r024-10-document-corpus-v1",
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

        events.append(
            {
                "event": "article_added",
                "timestamp": datetime.now(UTC).isoformat(),
                "article_ref": ref,
                "chunks_added": n_chunks,
                "network_fetch_attempted": False,
                "production_import_attempted": False,
                "graph_import_allowed": False,
                "ladybugdb_written": False,
            }
        )
        print(f"  + {ref}: {n_chunks} chunks")

    # Write graphml
    nx.write_graphml(graph, str(GRAPHML))

    # Compute graph statistics
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

    # Fail-closed invariants (M025 framework)
    summary = {
        "schema_version": "r024-networkx-probe-summary.v00.01",
        "generated_at": datetime.now(UTC).isoformat(),
        "corpus_size": len(articles),
        "n_nodes": n_nodes,
        "n_edges": n_edges,
        "node_types": node_types,
        "edge_types": edge_types,
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

    # Events
    with open(PROBE_EVENTS, "w") as f:
        for e in events:
            f.write(json.dumps(e) + "\n")

    print(f"summary: nodes={n_nodes}, edges={n_edges}")
    print(f"  node_types={node_types}")
    print(f"  edge_types={edge_types}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
