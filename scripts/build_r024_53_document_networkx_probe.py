#!/usr/bin/env python3
"""R024 S04: extended NetworkX graph probe at 53-article scale + memory profile.

Builds extended DiGraph:
- 1 corpus + 53 articles + 112 chunks + 265 entities (5 types x 53) = 431 nodes
- Edges: corpus->article (53), article->chunk (112), article->entity (265),
  article_cites_article via coarse_topic_code (~80+)

Memory profiling: tracemalloc snapshots before/after build.

NO LadybugDB. NO FalkorDB. NO Neo4j.

Outputs:
- data/r024-53-document-corpus-v1/networkx-probe/probe.graphml
- data/r024-53-document-corpus-v1/networkx-probe/summary.json
- data/r024-53-document-corpus-v1/networkx-probe/memory-profile.json
- data/r024-53-document-corpus-v1/networkx-probe/events.jsonl
"""

from __future__ import annotations

import json
import sys
import tracemalloc
from collections import defaultdict
from datetime import UTC, datetime
from pathlib import Path

REPO_ROOT = Path("/root/daily-archive")
R053_DIR = REPO_ROOT / "data" / "r024-53-document-corpus-v1"
PROBE_DIR = R053_DIR / "networkx-probe"
SELECTION = R053_DIR / "selection.json"
EVENTS_LOG = R053_DIR / "parser-chunking" / "events.jsonl"
GRAPHML = PROBE_DIR / "probe.graphml"
SUMMARY = PROBE_DIR / "summary.json"
MEMORY_PROFILE = PROBE_DIR / "memory-profile.json"
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
    """Find article_cites_article relations via coarse_topic_code."""
    relations: list[tuple[str, str, str]] = []
    by_cat: dict[str, list[str]] = defaultdict(list)
    for a in articles:
        ref = a["article_ref"]
        parts = ref.split("/")
        if len(parts) >= 2 and parts[0] == "arxiv":
            cat = parts[1]
        elif len(parts) >= 2 and parts[0] == "company_blog":
            cat = parts[1]
        elif len(parts) >= 2 and parts[0] == "nature":
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
            chunk_counts[str(ev["article_ref"])] = int(str(ev.get("chunk_count", 0)))

    # memory profile: before
    tracemalloc.start()
    snap_before = tracemalloc.take_snapshot()

    events: list[dict[str, object]] = []
    graph = nx.DiGraph(name="r024-53-document-corpus-extended-probe")

    graph.add_node(
        "corpus:r024-53-document-corpus-v1",
        node_type="corpus",
        label="R024 53-document corpus v1",
        scale=53,
    )

    for a in articles:
        ref = a["article_ref"]
        key = a["article_key"]
        n_chunks = chunk_counts.get(ref, 0)

        article_node_id = f"article:{ref}"
        graph.add_node(
            article_node_id,
            node_type="article",
            article_ref=ref,
            article_key=key,
            selection_role=a.get("selection_role", ""),
            source_code=a.get("source_code", ""),
            source_kind=a.get("source_kind", ""),
            topic_tags=",".join(
                a.get("topic_tags", []) if isinstance(a.get("topic_tags"), list) else []
            ),
        )
        graph.add_edge(
            "corpus:r024-53-document-corpus-v1",
            article_node_id,
            edge_type="corpus_contains_article",
        )

        for i in range(n_chunks):
            chunk_id = f"chunk:{ref}:{i + 1:04d}"
            graph.add_node(
                chunk_id,
                node_type="chunk",
                article_ref=ref,
                chunk_index=i + 1,
            )
            graph.add_edge(article_node_id, chunk_id, edge_type="article_contains_chunk")

        for entity_type in ENTITY_TYPES:
            entity_id = f"entity:{ref}:{entity_type}"
            graph.add_node(
                entity_id,
                node_type="entity",
                entity_type=entity_type,
                article_ref=ref,
                source="m025_chunk_types",
            )
            graph.add_edge(article_node_id, entity_id, edge_type="article_has_entity")

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

    # memory profile: after
    snap_after = tracemalloc.take_snapshot()
    stats_before = snap_before.compare_to(snap_after, "filename")
    stats_after = snap_after.compare_to(snap_before, "filename")

    # top allocations
    top_diff = snap_after.compare_to(snap_before, "lineno")
    top_5 = [
        {
            "file": str(stat.traceback),
            "size_diff_bytes": stat.size_diff,
            "size_bytes": stat.size,
        }
        for stat in top_diff[:5]
    ]

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

    # memory stats (tracemalloc)
    current, peak = tracemalloc.get_traced_memory()
    tracemalloc.stop()

    summary = {
        "schema_version": "r024-53-document-networkx-probe-summary.v00.01",
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

    memory_profile = {
        "schema_version": "r024-53-document-memory-profile.v00.01",
        "generated_at": datetime.now(UTC).isoformat(),
        "tracemalloc_current_bytes": current,
        "tracemalloc_peak_bytes": peak,
        "peak_mb": round(peak / (1024 * 1024), 4),
        "current_mb": round(current / (1024 * 1024), 4),
        "n_nodes": n_nodes,
        "n_edges": n_edges,
        "approx_bytes_per_node": current // max(1, n_nodes),
        "top_5_allocations": top_5,
        "method": "tracemalloc",
    }
    MEMORY_PROFILE.write_text(json.dumps(memory_profile, indent=2))

    with open(PROBE_EVENTS, "w") as f:
        for e in events:
            f.write(json.dumps(e) + "\n")

    print(f"summary: nodes={n_nodes}, edges={n_edges}")
    print(f"  node_types={node_types}")
    print(f"  edge_types={edge_types}")
    print(f"memory: peak={peak / (1024 * 1024):.2f} MB, current={current / (1024 * 1024):.2f} MB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
