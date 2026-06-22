#!/usr/bin/env python3
"""Estimate M061 2-hop BFS scale from the existing M058 one-hop manifest.

This is an algorithm-only preview. It does not acquire new papers, does not write
production graph state, and does not promote facts.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path
from typing import Any

from m060b_graph_stats import (
    DEFAULT_MANIFEST,
    DEFAULT_OUTPUT_DIR,
    LOOPBACK_BIND_HOST,
    SAFETY_DEFAULTS,
    build_graph,
    edge_layer,
    edge_source,
    edge_target,
    load_edges,
)

DEFAULT_ANCHOR = "2605.18747"
DEFAULT_OUTPUT = DEFAULT_OUTPUT_DIR / "two-hop-preview.json"


def assert_safety_defaults() -> None:
    """Fail closed if any M060b safety default is not explicitly false."""
    if LOOPBACK_BIND_HOST != "127.0.0.1":
        raise RuntimeError("Loopback bind host must remain 127.0.0.1")
    enabled = [name for name, value in SAFETY_DEFAULTS.items() if value is not False]
    if enabled:
        raise RuntimeError(f"Safety defaults must remain disabled: {enabled}")


def edge_key(edge: dict[str, Any]) -> tuple[str, str, str, str]:
    """Return a stable unique key for a manifest edge."""
    return (
        edge_source(edge),
        edge_target(edge),
        edge_layer(edge),
        str(edge.get("evidence_id") or ""),
    )


def compute_two_hop_preview(
    edges: list[dict[str, Any]], anchor: str = DEFAULT_ANCHOR
) -> dict[str, Any]:
    """Compute directed 1-hop and 2-hop preview counts from manifest edges."""
    assert_safety_defaults()
    graph = build_graph(edges)
    if anchor not in graph:
        raise ValueError(f"Anchor {anchor} is absent from the graph")

    outgoing_by_source: dict[str, list[dict[str, Any]]] = {}
    for edge in edges:
        outgoing_by_source.setdefault(edge_source(edge), []).append(edge)

    first_hop_edges = outgoing_by_source.get(anchor, [])
    first_hop_nodes = {edge_target(edge) for edge in first_hop_edges}

    traversed_edges: dict[tuple[str, str, str, str], dict[str, Any]] = {
        edge_key(edge): edge for edge in first_hop_edges
    }
    second_hop_targets: set[str] = set()
    second_hop_edge_count = 0
    for node in sorted(first_hop_nodes):
        for edge in outgoing_by_source.get(node, []):
            traversed_edges[edge_key(edge)] = edge
            second_hop_edge_count += 1
            target = edge_target(edge)
            if target != anchor and target not in first_hop_nodes:
                second_hop_targets.add(target)

    layer_counts = Counter(edge_layer(edge) for edge in traversed_edges.values())
    second_layer_counts = Counter(
        edge_layer(edge) for node in first_hop_nodes for edge in outgoing_by_source.get(node, [])
    )

    return {
        "schema_version": "m060b.two_hop_preview.v1",
        "mode": "algorithm_only_preview_not_acquisition",
        "anchor": anchor,
        "source_manifest_edges": len(edges),
        "source_graph_nodes": graph.number_of_nodes(),
        "source_graph_edges": graph.number_of_edges(),
        "one_hop_unique_nodes": len(first_hop_nodes),
        "one_hop_unique_edges": len({edge_key(edge) for edge in first_hop_edges}),
        "two_hop_new_unique_nodes": len(second_hop_targets),
        "two_hop_unique_edges": len(traversed_edges),
        "two_hop_second_frontier_edges": second_hop_edge_count,
        "m061_estimated_new_nodes": len(second_hop_targets),
        "m061_estimated_edges": len(traversed_edges),
        "per_layer_two_hop_edge_counts": dict(sorted(layer_counts.items())),
        "per_layer_second_frontier_edge_counts": dict(sorted(second_layer_counts.items())),
        "safety_defaults": dict(SAFETY_DEFAULTS),
        "loopback_bind_host": LOOPBACK_BIND_HOST,
    }


def write_preview(preview: dict[str, Any], output_path: Path) -> None:
    """Write the preview JSON artifact."""
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(preview, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def run_preview(
    manifest_path: Path, output_path: Path, anchor: str = DEFAULT_ANCHOR
) -> dict[str, Any]:
    """Load the manifest, compute the preview, and write JSON."""
    edges = load_edges(manifest_path)
    preview = compute_two_hop_preview(edges, anchor=anchor)
    write_preview(preview, output_path)
    return preview


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--anchor", default=DEFAULT_ANCHOR)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    preview = run_preview(args.manifest, args.output, anchor=args.anchor)
    print(
        "Computed M060b 2-hop preview "
        f"for anchor {preview['anchor']} to {args.output} "
        f"({preview['one_hop_unique_nodes']} one-hop nodes, "
        f"{preview['two_hop_new_unique_nodes']} new 2-hop nodes, "
        f"{preview['m061_estimated_edges']} estimated edges)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
