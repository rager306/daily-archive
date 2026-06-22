#!/usr/bin/env python3
"""Render a readable PNG preview of the M058 four-layer graph.

This tool is read-only and diagnostic-only. It prefers matplotlib for drawing
when available, and falls back to a small stdlib PNG renderer so the M060b
visualization remains reproducible in environments where optional plotting
packages are absent.
"""

from __future__ import annotations

import argparse
import math
import struct
import sys
import zlib
from binascii import crc32
from pathlib import Path
from typing import Any

import networkx as nx
from m060b_graph_stats import (
    DEFAULT_MANIFEST,
    DEFAULT_OUTPUT_DIR,
    LAYER_ORDER,
    LOOPBACK_BIND_HOST,
    SAFETY_DEFAULTS,
    build_graph,
    load_edges,
)

DEFAULT_OUTPUT = DEFAULT_OUTPUT_DIR / "graph-viz.png"
MAX_VISUALIZATION_NODES = 200
LAYOUT_SEED = 42
DEFAULT_EDGE_ALPHA = 0.3
BACKGROUND_RGB = (255, 255, 255)
NODE_RGB = (248, 250, 252)
NODE_BORDER_RGB = (15, 23, 42)
LAYER_COLORS: dict[str, str] = {
    "citation": "#2563eb",
    "table_similarity": "#16a34a",
    "figure_similarity_v1": "#f97316",
    "figure_similarity_v2": "#dc2626",
}
LAYER_RGB: dict[str, tuple[int, int, int]] = {
    "citation": (37, 99, 235),
    "table_similarity": (22, 163, 74),
    "figure_similarity_v1": (249, 115, 22),
    "figure_similarity_v2": (220, 38, 38),
}


def assert_safety_defaults() -> None:
    """Fail closed if any safety default is not explicitly false."""
    if LOOPBACK_BIND_HOST != "127.0.0.1":
        raise RuntimeError("Loopback bind host must remain 127.0.0.1")
    enabled = [name for name, value in SAFETY_DEFAULTS.items() if value is not False]
    if enabled:
        raise RuntimeError(f"Safety defaults must remain disabled: {enabled}")


def edge_alpha(similarity: Any) -> float:
    """Return an edge alpha from similarity, or the diagnostic default."""
    if isinstance(similarity, int | float):
        if math.isnan(float(similarity)):
            return DEFAULT_EDGE_ALPHA
        return max(0.05, min(1.0, float(similarity)))
    return DEFAULT_EDGE_ALPHA


def top_degree_subgraph(graph: nx.DiGraph, max_nodes: int = MAX_VISUALIZATION_NODES) -> nx.DiGraph:
    """Return a top-degree induced subgraph capped for PNG readability."""
    if graph.number_of_nodes() <= max_nodes:
        return graph.copy()
    top_nodes = [
        node
        for node, _degree in sorted(
            graph.degree(), key=lambda item: (-int(item[1]), str(item[0]))
        )[:max_nodes]
    ]
    return graph.subgraph(top_nodes).copy()


def degree_scaled_node_sizes(graph: nx.DiGraph) -> list[float]:
    """Scale node area by degree while keeping outliers readable."""
    degrees = dict(graph.degree())
    max_degree = max(degrees.values(), default=1)
    return [70.0 + 430.0 * (degrees[node] / max_degree) for node in graph.nodes]


def render_with_matplotlib(graph: nx.DiGraph, output_path: Path) -> str:
    """Render the graph with matplotlib and NetworkX spring_layout."""
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from matplotlib.lines import Line2D

    positions = nx.spring_layout(graph, seed=LAYOUT_SEED)
    edge_layers = [str(data.get("layer", "citation")) for _u, _v, data in graph.edges(data=True)]
    edge_colors = [LAYER_COLORS.get(layer, "#64748b") for layer in edge_layers]
    edge_alphas = [edge_alpha(data.get("similarity")) for _u, _v, data in graph.edges(data=True)]

    fig, ax = plt.subplots(figsize=(14, 10), dpi=160)
    ax.set_title(
        f"M060b four-layer graph preview: {graph.number_of_nodes()} nodes, "
        f"{graph.number_of_edges()} edges"
    )
    ax.axis("off")

    nx.draw_networkx_edges(
        graph,
        positions,
        edge_color=edge_colors,
        alpha=edge_alphas,
        arrows=False,
        width=0.8,
        ax=ax,
    )
    nx.draw_networkx_nodes(
        graph,
        positions,
        node_size=degree_scaled_node_sizes(graph),
        node_color="#f8fafc",
        edgecolors="#0f172a",
        linewidths=0.35,
        ax=ax,
    )

    legend_handles = [
        Line2D([0], [0], color=LAYER_COLORS[layer], lw=2, label=layer)
        for layer in LAYER_ORDER
    ]
    ax.legend(handles=legend_handles, loc="lower left", frameon=True, fontsize=8)
    fig.tight_layout()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(output_path, bbox_inches="tight")
    plt.close(fig)
    return "matplotlib"


def _png_chunk(chunk_type: bytes, payload: bytes) -> bytes:
    return (
        struct.pack(">I", len(payload))
        + chunk_type
        + payload
        + struct.pack(">I", crc32(chunk_type + payload) & 0xFFFFFFFF)
    )


def _write_png(output_path: Path, width: int, height: int, pixels: bytearray) -> None:
    rows = bytearray()
    stride = width * 3
    for y in range(height):
        rows.append(0)
        rows.extend(pixels[y * stride : (y + 1) * stride])
    payload = b"".join(
        [
            b"\x89PNG\r\n\x1a\n",
            _png_chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)),
            _png_chunk(b"IDAT", zlib.compress(bytes(rows), level=9)),
            _png_chunk(b"IEND", b""),
        ]
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_bytes(payload)


def _blend_pixel(
    pixels: bytearray,
    width: int,
    height: int,
    x: int,
    y: int,
    color: tuple[int, int, int],
    alpha: float,
) -> None:
    if not (0 <= x < width and 0 <= y < height):
        return
    idx = (y * width + x) * 3
    inv = 1.0 - alpha
    pixels[idx] = int(pixels[idx] * inv + color[0] * alpha)
    pixels[idx + 1] = int(pixels[idx + 1] * inv + color[1] * alpha)
    pixels[idx + 2] = int(pixels[idx + 2] * inv + color[2] * alpha)


def _draw_line(
    pixels: bytearray,
    width: int,
    height: int,
    start: tuple[int, int],
    end: tuple[int, int],
    color: tuple[int, int, int],
    alpha: float,
) -> None:
    x0, y0 = start
    x1, y1 = end
    dx = abs(x1 - x0)
    dy = -abs(y1 - y0)
    sx = 1 if x0 < x1 else -1
    sy = 1 if y0 < y1 else -1
    err = dx + dy
    while True:
        _blend_pixel(pixels, width, height, x0, y0, color, alpha)
        if x0 == x1 and y0 == y1:
            break
        twice_err = 2 * err
        if twice_err >= dy:
            err += dy
            x0 += sx
        if twice_err <= dx:
            err += dx
            y0 += sy


def _draw_circle(
    pixels: bytearray,
    width: int,
    height: int,
    center: tuple[int, int],
    radius: int,
    color: tuple[int, int, int],
    alpha: float,
) -> None:
    cx, cy = center
    radius_sq = radius * radius
    for y in range(cy - radius, cy + radius + 1):
        for x in range(cx - radius, cx + radius + 1):
            if (x - cx) ** 2 + (y - cy) ** 2 <= radius_sq:
                _blend_pixel(pixels, width, height, x, y, color, alpha)


def _scale_positions(
    positions: dict[Any, Any],
    width: int,
    height: int,
    margin: int,
) -> dict[Any, tuple[int, int]]:
    xs = [float(value[0]) for value in positions.values()]
    ys = [float(value[1]) for value in positions.values()]
    min_x, max_x = min(xs, default=-1.0), max(xs, default=1.0)
    min_y, max_y = min(ys, default=-1.0), max(ys, default=1.0)
    span_x = max(max_x - min_x, 1e-9)
    span_y = max(max_y - min_y, 1e-9)
    scaled: dict[Any, tuple[int, int]] = {}
    for node, value in positions.items():
        x = margin + (float(value[0]) - min_x) / span_x * (width - 2 * margin)
        y = margin + (float(value[1]) - min_y) / span_y * (height - 2 * margin)
        scaled[node] = (int(x), int(height - y))
    return scaled


def render_with_stdlib_png(graph: nx.DiGraph, output_path: Path) -> str:
    """Render a deterministic PNG without optional plotting packages."""
    width, height, margin = 1200, 850, 70
    pixels = bytearray(BACKGROUND_RGB * width * height)
    positions = nx.spring_layout(graph, seed=LAYOUT_SEED)
    scaled_positions = _scale_positions(positions, width, height, margin)
    degrees = dict(graph.degree())
    max_degree = max(degrees.values(), default=1)

    for source, target, data in graph.edges(data=True):
        if source not in scaled_positions or target not in scaled_positions:
            continue
        layer = str(data.get("layer", "citation"))
        _draw_line(
            pixels,
            width,
            height,
            scaled_positions[source],
            scaled_positions[target],
            LAYER_RGB.get(layer, (100, 116, 139)),
            edge_alpha(data.get("similarity")),
        )

    for node, center in scaled_positions.items():
        radius = 3 + int(9 * (degrees.get(node, 0) / max_degree))
        _draw_circle(pixels, width, height, center, radius + 1, NODE_BORDER_RGB, 0.9)
        _draw_circle(pixels, width, height, center, radius, NODE_RGB, 1.0)

    _write_png(output_path, width, height, pixels)
    return "stdlib_png_fallback"


def render_graph(manifest_path: Path, output_path: Path, max_nodes: int = MAX_VISUALIZATION_NODES) -> dict[str, Any]:
    """Load, subsample, and render the M060b graph preview PNG."""
    assert_safety_defaults()
    edges = load_edges(manifest_path)
    graph = build_graph(edges)
    preview_graph = top_degree_subgraph(graph, max_nodes=max_nodes)
    try:
        renderer = render_with_matplotlib(preview_graph, output_path)
    except ModuleNotFoundError as exc:
        if exc.name != "matplotlib":
            raise
        renderer = render_with_stdlib_png(preview_graph, output_path)
    return {
        "renderer": renderer,
        "source_nodes": graph.number_of_nodes(),
        "source_edges": graph.number_of_edges(),
        "rendered_nodes": preview_graph.number_of_nodes(),
        "rendered_edges": preview_graph.number_of_edges(),
        "output": str(output_path),
        "safety_defaults": dict(SAFETY_DEFAULTS),
        "loopback_bind_host": LOOPBACK_BIND_HOST,
    }


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--max-nodes", type=int, default=MAX_VISUALIZATION_NODES)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    result = render_graph(args.manifest, args.output, max_nodes=args.max_nodes)
    print(
        "Rendered M060b graph preview "
        f"with {result['renderer']} to {result['output']} "
        f"({result['rendered_nodes']} nodes, {result['rendered_edges']} edges)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
