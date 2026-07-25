#!/usr/bin/env python3
"""Compute NetworkX statistics for the M058 four-layer graph manifest.

The tool is read-only and diagnostic-only. It builds a NetworkX DiGraph over
artifact IDs so table/figure intra-document links remain real edges instead of
paper-level self-loops, while per-layer paper statistics are reported
separately.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path
from statistics import mean
from typing import Any

import networkx as nx

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "artifacts" / "m058-pilot" / "combined-edges.json"
DEFAULT_OUTPUT_DIR = ROOT / "artifacts" / "m060b-graph"
LOOPBACK_BIND_HOST = "127.0.0.1"

SAFETY_DEFAULTS: dict[str, bool] = {
    "external_network_authorized": False,
    "fact_promotion_authorized": False,
    "graph_writes_authorized": False,
    "llm_calls_authorized": False,
    "production_import_authorized": False,
}

LAYER_ORDER = ("citation", "table_similarity", "figure_similarity_v1", "figure_similarity_v2")


def read_json(path: Path) -> dict[str, Any]:
    """Read a JSON object from disk."""
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"Expected JSON object in {path}")
    return payload


def load_edges(manifest_path: Path) -> list[dict[str, Any]]:
    """Load edge dictionaries from the graph manifest."""
    payload = read_json(manifest_path)
    edges = payload.get("edges")
    if not isinstance(edges, list):
        raise ValueError(f"Manifest {manifest_path} does not contain an edges array")
    return edges


def edge_source(edge: dict[str, Any]) -> str:
    """Return the artifact-level source node for one edge."""
    source = edge.get("source_artifact_id")
    if not isinstance(source, str) or not source:
        raise ValueError(f"Edge is missing source_artifact_id: {edge!r}")
    return source


def edge_target(edge: dict[str, Any]) -> str:
    """Return the artifact-level target node for one edge."""
    target = edge.get("target_artifact_id")
    if not isinstance(target, str) or not target:
        raise ValueError(f"Edge is missing target_artifact_id: {edge!r}")
    return target


def edge_layer(edge: dict[str, Any]) -> str:
    """Return the evidence layer for one edge."""
    layer = edge.get("evidence_layer")
    if not isinstance(layer, str) or not layer:
        raise ValueError(f"Edge is missing evidence_layer: {edge!r}")
    return layer


def build_graph(edges: list[dict[str, Any]]) -> nx.DiGraph:
    """Build a NetworkX DiGraph with manifest edge attributes."""
    graph = nx.DiGraph()
    for edge in edges:
        source = edge_source(edge)
        target = edge_target(edge)
        graph.add_edge(
            source,
            target,
            layer=edge_layer(edge),
            similarity=edge.get("similarity_score"),
            evidence_id=edge.get("evidence_id"),
            source_paper_id=edge.get("source_paper_id"),
            target_paper_id=edge.get("target_paper_id"),
            relation_type=edge.get("relation_type"),
        )
    return graph


def _sorted_degree_items(items: Any, limit: int = 10) -> list[dict[str, int | str]]:
    sorted_items = sorted(items, key=lambda item: (-item[1], str(item[0])))[:limit]
    return [{"node": str(node), "degree": int(degree)} for node, degree in sorted_items]


def _sorted_counter_items(counter: Counter[Any], limit: int = 10) -> list[dict[str, int]]:
    # pyrefly: ignore [bad-return]
    return [
        {"id": str(key), "count": int(count)}
        for key, count in sorted(counter.items(), key=lambda item: (-item[1], str(item[0])))[:limit]
    ]  # ty:ignore[invalid-return-type]


def compute_layer_stats(edges: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    """Compute per-layer edge, paper, and similarity statistics."""
    by_layer: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for edge in edges:
        by_layer[edge_layer(edge)].append(edge)

    stats: dict[str, dict[str, Any]] = {}
    for layer in LAYER_ORDER:
        layer_edges = by_layer.get(layer, [])
        source_papers = {
            str(edge.get("source_paper_id")) for edge in layer_edges if edge.get("source_paper_id")
        }
        target_papers = {
            str(edge.get("target_paper_id")) for edge in layer_edges if edge.get("target_paper_id")
        }
        similarities = [
            float(edge["similarity_score"])
            for edge in layer_edges
            if isinstance(edge.get("similarity_score"), int | float)
        ]
        stats[layer] = {
            "edge_count": len(layer_edges),
            "distinct_source_papers": len(source_papers),
            "distinct_target_papers": len(target_papers),
            "distinct_papers": len(source_papers | target_papers),
            "mean_similarity": round(mean(similarities), 6) if similarities else None,
        }
    return stats


def compute_multi_edges(edges: list[dict[str, Any]]) -> dict[str, Any]:
    """Count repeated edge pairs at artifact and paper granularity."""
    artifact_pairs = Counter((edge_source(edge), edge_target(edge)) for edge in edges)
    paper_pairs = Counter(
        (str(edge.get("source_paper_id")), str(edge.get("target_paper_id")))
        for edge in edges
        if edge.get("source_paper_id") and edge.get("target_paper_id")
    )
    artifact_multi = Counter({pair: count for pair, count in artifact_pairs.items() if count > 1})
    paper_multi = Counter({pair: count for pair, count in paper_pairs.items() if count > 1})
    return {
        "artifact_pair_count": len(artifact_multi),
        "top_artifact_pairs": [
            {"source": source, "target": target, "count": int(count)}
            for (source, target), count in artifact_multi.most_common(10)
        ],
        "paper_pair_count": len(paper_multi),
        "top_paper_pairs": [
            {"source": source, "target": target, "count": int(count)}
            for (source, target), count in paper_multi.most_common(10)
        ],
    }


def compute_citation_top_targets(
    edges: list[dict[str, Any]], limit: int = 10
) -> list[dict[str, int | str]]:
    """Return the top cited target papers from citation-layer edges."""
    citation_targets = Counter(
        str(edge.get("target_paper_id"))
        for edge in edges
        if edge.get("evidence_layer") == "citation" and edge.get("target_paper_id")
    )
    # pyrefly: ignore [bad-return]
    return _sorted_counter_items(citation_targets, limit=limit)  # ty:ignore[invalid-return-type]


def compute_stats(manifest_path: Path) -> dict[str, Any]:
    """Compute the full graph statistics payload."""
    manifest = read_json(manifest_path)
    edges = manifest.get("edges")
    if not isinstance(edges, list):
        raise ValueError(f"Manifest {manifest_path} does not contain an edges array")

    graph = build_graph(edges)
    weak_components = list(nx.weakly_connected_components(graph))
    strong_components = list(nx.strongly_connected_components(graph))
    self_loops = list(nx.selfloop_edges(graph))
    orphan_nodes = [str(node) for node, degree in graph.degree() if degree == 0]

    return {
        "manifest_path": str(manifest_path),
        "schema_version": manifest.get("schema_version"),
        "loopback_bind_host": manifest.get("loopback_bind_host", LOOPBACK_BIND_HOST),
        "safety_defaults": manifest.get("safety_defaults"),
        "expected_safety_defaults": SAFETY_DEFAULTS,
        "manifest_edge_count": manifest.get("edge_count"),
        "total_nodes": graph.number_of_nodes(),
        "total_edges": len(edges),
        "networkx_graph_edges": graph.number_of_edges(),
        "density": nx.density(graph),
        "per_layer": compute_layer_stats(edges),
        "degree_distribution": {
            "top_in_degree": _sorted_degree_items(graph.in_degree()),
            "top_out_degree": _sorted_degree_items(graph.out_degree()),
            "top_total_degree": _sorted_degree_items(graph.degree()),
            "top_citation_target_papers": compute_citation_top_targets(edges),
        },
        "connected_components": {
            "weakly_connected_count": len(weak_components),
            "largest_weakly_connected_size": max(
                (len(component) for component in weak_components), default=0
            ),
            "strongly_connected_count": len(strong_components),
            "largest_strongly_connected_size": max(
                (len(component) for component in strong_components), default=0
            ),
        },
        "orphans": {"count": len(orphan_nodes), "nodes": orphan_nodes[:50]},
        "self_loops": {
            "count": len(self_loops),
            "edges": [
                {"source": str(source), "target": str(target)} for source, target in self_loops[:50]
            ],
        },
        "multi_edges": compute_multi_edges(edges),
    }


def resolve_output_paths(output: Path | None) -> tuple[Path, Path]:
    """Resolve JSON and Markdown output paths from an optional CLI target."""
    if output is None:
        output_dir = DEFAULT_OUTPUT_DIR
        return output_dir / "stats.json", output_dir / "stats.md"
    if output.suffix == ".json":
        return output, output.with_suffix(".md")
    return output / "stats.json", output / "stats.md"


def write_stats(stats: dict[str, Any], json_path: Path, md_path: Path) -> None:
    """Write JSON and Markdown stats reports idempotently."""
    json_path.parent.mkdir(parents=True, exist_ok=True)
    md_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(json.dumps(stats, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    md_path.write_text(render_markdown(stats), encoding="utf-8")


def render_markdown(stats: dict[str, Any]) -> str:
    """Render a compact Markdown statistics report."""
    lines = [
        "# M060b NetworkX Graph Statistics",
        "",
        "This report is read-only. Production import is not authorized. Graph writes are disabled.",
        f"Loopback bind host: `{stats['loopback_bind_host']}`.",
        "",
        "## Totals",
        "",
        f"- Total nodes: {stats['total_nodes']}",
        f"- Total edges: {stats['total_edges']}",
        f"- NetworkX graph edges: {stats['networkx_graph_edges']}",
        f"- Density: {stats['density']:.12f}",
        f"- Self-loops: {stats['self_loops']['count']}",
        f"- Orphans: {stats['orphans']['count']}",
        f"- Artifact multi-edge pairs: {stats['multi_edges']['artifact_pair_count']}",
        f"- Paper multi-edge pairs: {stats['multi_edges']['paper_pair_count']}",
        "",
        "## Per-layer Statistics",
        "",
        "| Layer | Edges | Source papers | Target papers | Distinct papers | Mean similarity |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    for layer, layer_stats in stats["per_layer"].items():
        mean_similarity = layer_stats["mean_similarity"]
        mean_text = "n/a" if mean_similarity is None else f"{mean_similarity:.6f}"
        lines.append(
            "| {layer} | {edges} | {sources} | {targets} | {papers} | {mean} |".format(
                layer=layer,
                edges=layer_stats["edge_count"],
                sources=layer_stats["distinct_source_papers"],
                targets=layer_stats["distinct_target_papers"],
                papers=layer_stats["distinct_papers"],
                mean=mean_text,
            )
        )

    lines.extend(
        [
            "",
            "## Connected Components",
            "",
            f"- Weakly connected components: {stats['connected_components']['weakly_connected_count']}",
            f"- Largest weakly connected component size: {stats['connected_components']['largest_weakly_connected_size']}",
            f"- Strongly connected components: {stats['connected_components']['strongly_connected_count']}",
            f"- Largest strongly connected component size: {stats['connected_components']['largest_strongly_connected_size']}",
            "",
            "## Top Citation Target Papers",
            "",
        ]
    )
    for item in stats["degree_distribution"]["top_citation_target_papers"][:10]:
        lines.append(f"- `{item['id']}`: {item['count']} citation-layer incoming edges")

    lines.extend(["", "## Top Artifact Degrees", "", "### In-degree", ""])
    for item in stats["degree_distribution"]["top_in_degree"]:
        lines.append(f"- `{item['node']}`: {item['degree']}")
    lines.extend(["", "### Out-degree", ""])
    for item in stats["degree_distribution"]["top_out_degree"]:
        lines.append(f"- `{item['node']}`: {item['degree']}")
    lines.extend(["", "### Total degree", ""])
    for item in stats["degree_distribution"]["top_total_degree"]:
        lines.append(f"- `{item['node']}`: {item['degree']}")

    lines.extend(
        [
            "",
            "## Safety Defaults",
            "",
        ]
    )
    for key in sorted(SAFETY_DEFAULTS):
        lines.append(f"- `{key}` is disabled: `{stats['expected_safety_defaults'][key]}`")
    return "\n".join(lines) + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest", type=Path, default=DEFAULT_MANIFEST, help="Graph manifest JSON path"
    )
    parser.add_argument(
        "--output", type=Path, default=None, help="Output directory or stats JSON path"
    )
    return parser.parse_args()


def display_path(path: Path) -> str:
    """Return a stable display path for repository-relative or absolute outputs."""
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def main() -> int:
    args = parse_args()
    manifest_path = args.manifest if args.manifest.is_absolute() else ROOT / args.manifest
    stats = compute_stats(manifest_path)
    json_path, md_path = resolve_output_paths(args.output)
    if not json_path.is_absolute():
        json_path = ROOT / json_path
    if not md_path.is_absolute():
        md_path = ROOT / md_path
    write_stats(stats, json_path, md_path)
    sys.stdout.write(f"Wrote {display_path(json_path)}\n")
    sys.stdout.write(f"Wrote {display_path(md_path)}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
