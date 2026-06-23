#!/usr/bin/env python3
"""M121 S05: in-memory NetworkX probe for source-backed 219-record corpus.

Consumes S04 parser/chunking events. Completed records become article/chunk/entity
nodes; metadata-only records remain explicit exclusions. This is a bounded graph
probe only: no LadybugDB, FalkorDB, Neo4j, network, or production writes.
"""

from __future__ import annotations

import json
import sys
import tracemalloc
from collections import Counter, defaultdict
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

REPO_ROOT = Path("/root/daily-archive")
R218_DIR = REPO_ROOT / "data" / "r024-218-document-corpus-v1"
PARSER_EVENTS = R218_DIR / "parser-chunking" / "events.jsonl"
PROBE_DIR = R218_DIR / "networkx-probe"
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
    import networkx as nx
except ImportError:
    print("networkx not installed", file=sys.stderr)
    sys.exit(1)


def _load_events(path: Path) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    completed: list[dict[str, Any]] = []
    skipped: list[dict[str, Any]] = []
    for line in path.read_text().splitlines():
        if not line.strip():
            continue
        event = json.loads(line)
        if event.get("event") == "parser_chunking_complete":
            completed.append(event)
        elif event.get("event") == "parser_chunking_skipped_metadata_only":
            skipped.append(event)
    return completed, skipped


def _coarse_category(article_ref: str) -> str:
    parts = article_ref.split("/")
    if len(parts) >= 2 and parts[0] in {"arxiv", "company_blog", "nature"}:
        return parts[1]
    if len(parts) >= 2:
        return parts[0]
    return "other"


def find_citation_relations(events: list[dict[str, Any]]) -> list[tuple[str, str, str]]:
    """Find coarse article_cites_article relations grouped by source/category."""
    relations: list[tuple[str, str, str]] = []
    by_cat: dict[str, list[str]] = defaultdict(list)
    for event in events:
        ref = str(event["article_ref"])
        by_cat[_coarse_category(ref)].append(ref)

    seen: set[tuple[str, str]] = set()
    for cat, refs in sorted(by_cat.items()):
        if len(refs) < 2:
            continue
        for i, r1 in enumerate(sorted(refs)):
            for r2 in sorted(refs)[i + 1 :]:
                pair = (r1, r2) if r1 < r2 else (r2, r1)
                if pair in seen:
                    continue
                seen.add(pair)
                relations.append((r1, r2, cat))
    return relations


def _count_node_types(graph: Any) -> dict[str, int]:
    node_types: dict[str, int] = {}
    for _, data in graph.nodes(data=True):
        nt = str(data.get("node_type", "unknown")) if isinstance(data, dict) else "unknown"
        node_types[nt] = node_types.get(nt, 0) + 1
    return node_types


def _count_edge_types(graph: Any) -> dict[str, int]:
    edge_types: dict[str, int] = {}
    for _, _, data in graph.edges(data=True):
        et = str(data.get("edge_type", "unknown")) if isinstance(data, dict) else "unknown"
        edge_types[et] = edge_types.get(et, 0) + 1
    return edge_types


def main() -> int:
    PROBE_DIR.mkdir(parents=True, exist_ok=True)
    completed, skipped = _load_events(PARSER_EVENTS)
    print(
        "Building NetworkX probe for "
        f"{len(completed)} completed records ({len(skipped)} metadata-only skips)..."
    )

    tracemalloc.start()
    snap_before = tracemalloc.take_snapshot()

    graph = nx.DiGraph(name="r024-218-document-corpus-source-backed-probe")
    graph.add_node(
        "corpus:r024-218-document-corpus-v1",
        node_type="corpus",
        label="R024 218-document source-backed corpus v1",
        scale=len(completed),
        skipped_metadata_only=len(skipped),
    )

    probe_events: list[dict[str, Any]] = []
    chunk_count_total = 0
    source_kind_counts: Counter[str] = Counter()

    for event in completed:
        ref = str(event["article_ref"])
        key = str(event["article_key"])
        n_chunks = int(event["chunk_count"])
        source_kind = str(event.get("source_kind", "unknown"))
        chunk_count_total += n_chunks
        source_kind_counts[source_kind] += 1

        article_node_id = f"article:{ref}"
        graph.add_node(
            article_node_id,
            node_type="article",
            article_ref=ref,
            article_key=key,
            source_kind=source_kind,
            text_source=str(event.get("text_source", "")),
            chunk_count=n_chunks,
        )
        graph.add_edge(
            "corpus:r024-218-document-corpus-v1",
            article_node_id,
            edge_type="corpus_contains_article",
        )

        for index in range(n_chunks):
            chunk_id = f"chunk:{ref}:{index + 1:04d}"
            graph.add_node(
                chunk_id,
                node_type="chunk",
                article_ref=ref,
                chunk_index=index + 1,
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

        probe_events.append(
            {
                "event": "article_added",
                "timestamp": datetime.now(UTC).isoformat(),
                "article_ref": ref,
                "article_key": key,
                "chunks_added": n_chunks,
                "entities_added": len(ENTITY_TYPES),
                "network_fetch_attempted": False,
                "production_import_attempted": False,
                "graph_import_allowed": False,
                "ladybugdb_written": False,
            }
        )

    excluded_records = []
    for event in skipped:
        ref = str(event["article_ref"])
        key = str(event["article_key"])
        skip_reason = str(event.get("skip_reason", "metadata_only_no_local_source_artifact"))
        excluded_records.append(
            {"article_ref": ref, "article_key": key, "skip_reason": skip_reason}
        )
        probe_events.append(
            {
                "event": "metadata_only_excluded",
                "timestamp": datetime.now(UTC).isoformat(),
                "article_ref": ref,
                "article_key": key,
                "skip_reason": skip_reason,
                "network_fetch_attempted": False,
                "production_import_attempted": False,
                "graph_import_allowed": False,
                "ladybugdb_written": False,
            }
        )

    citations = find_citation_relations(completed)
    print(f"  citation relations: {len(citations)}")
    for r1, r2, category in citations:
        graph.add_edge(
            f"article:{r1}",
            f"article:{r2}",
            edge_type="article_cites_article",
            source=f"coarse_category:{category}",
        )

    nx.write_graphml(graph, str(GRAPHML))

    snap_after = tracemalloc.take_snapshot()
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
    node_types = _count_node_types(graph)
    edge_types = _count_edge_types(graph)

    current, peak = tracemalloc.get_traced_memory()
    tracemalloc.stop()

    fail_closed_invariants = {
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "graph_import_allowed": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_readiness_claim": False,
        "falkordb_written": False,
        "neo4j_written": False,
        "ladybugdb_connection_attempted": False,
    }

    summary = {
        "schema_version": "r024-218-document-networkx-probe-summary.v00.01",
        "generated_at": datetime.now(UTC).isoformat(),
        "total_catalog_records_seen": len(completed) + len(skipped),
        "corpus_size": len(completed),
        "skipped_metadata_only": len(skipped),
        "excluded_records": excluded_records,
        "chunk_count_total": chunk_count_total,
        "source_kind_counts": dict(sorted(source_kind_counts.items())),
        "n_nodes": n_nodes,
        "n_edges": n_edges,
        "node_types": node_types,
        "edge_types": edge_types,
        "citation_relations_count": len(citations),
        "entity_types": list(ENTITY_TYPES),
        "fail_closed_invariants": fail_closed_invariants,
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
        "schema_version": "r024-218-document-memory-profile.v00.01",
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
        for event in probe_events:
            f.write(json.dumps(event) + "\n")

    print(f"summary: nodes={n_nodes}, edges={n_edges}")
    print(f"  node_types={node_types}")
    print(f"  edge_types={edge_types}")
    print(f"memory: peak={peak / (1024 * 1024):.2f} MB, current={current / (1024 * 1024):.2f} MB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
