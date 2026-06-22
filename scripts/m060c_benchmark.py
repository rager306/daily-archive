#!/usr/bin/env python3
"""Benchmark NetworkX, igraph, and rustworkx for M060c graph-library research."""

from __future__ import annotations

import argparse
import importlib
import json
import math
import statistics
import time
from collections.abc import Callable, Iterable, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import networkx as nx

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "artifacts" / "m058-pilot" / "combined-edges.json"
DEFAULT_OUTPUT_DIR = ROOT / "artifacts" / "m060c-benchmark"
LOOPBACK_HOST = "127.0.0.1"
RUNS_PER_OPERATION = 5
ALGORITHMS = ("bfs", "pagerank", "shortest_path", "connected_components")
LIBRARIES = ("networkx", "igraph", "rustworkx")
SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_enabled": False,
    "llm_calls_enabled": False,
}
SAFETY_STATEMENTS = (
    "Graph writes are not authorized.",
    "Production import is not authorized.",
    "Fact promotion is not authorized.",
    "External network default is disabled.",
    "LLM calls default is disabled.",
)


@dataclass(frozen=True)
class GraphSpec:
    """Prepared benchmark graph plus deterministic anchor points."""

    name: str
    graph: nx.DiGraph
    source: Any
    target: Any

    @property
    def node_count(self) -> int:
        return self.graph.number_of_nodes()

    @property
    def edge_count(self) -> int:
        return self.graph.number_of_edges()


@dataclass(frozen=True)
class LibraryAdapter:
    """A converted graph plus algorithm callables for one library."""

    name: str
    status: str
    graph: Any | None
    error: str | None
    operations: dict[str, Callable[[], Any]]


def load_manifest_graph(manifest_path: Path) -> nx.DiGraph:
    """Load the M058 four-layer graph manifest as a weighted DiGraph."""

    payload = json.loads(manifest_path.read_text(encoding="utf-8"))
    graph = nx.DiGraph()
    for edge in payload.get("edges", []):
        source = edge["source_artifact_id"]
        target = edge["target_artifact_id"]
        score = edge.get("similarity_score")
        weight = float(score) if isinstance(score, int | float) and score > 0 else 1.0
        graph.add_edge(
            source,
            target,
            weight=weight,
            evidence_layer=edge.get("evidence_layer"),
            relation_type=edge.get("relation_type"),
        )
    return graph


def synthetic_graph(edge_count: int, *, density: float, seed: int) -> nx.DiGraph:
    """Build a deterministic directed weighted Erdos-Renyi-style graph."""

    node_count = max(2, math.ceil((edge_count / density) ** 0.5)) if density > 0 else edge_count
    graph = nx.gnm_random_graph(node_count, edge_count, seed=seed, directed=True)
    graph = nx.relabel_nodes(graph, {node: f"synthetic-{node}" for node in graph.nodes})
    for index, (source, target) in enumerate(graph.edges):
        graph[source][target]["weight"] = 1.0 + (index % 100) / 100.0
    return graph


def choose_path_pair(graph: nx.DiGraph) -> tuple[Any, Any]:
    """Choose a deterministic source-target pair that has at least a direct path."""

    try:
        return next(iter(graph.edges))
    except StopIteration as exc:
        raise ValueError("benchmark graph must contain at least one edge") from exc


def build_graph_specs(
    manifest_path: Path, synthetic_edges: Sequence[int] = (10_000, 100_000)
) -> list[GraphSpec]:
    """Prepare the real graph and synthetic graphs for benchmarking."""

    real_graph = load_manifest_graph(manifest_path)
    if real_graph.number_of_nodes() < 2 or real_graph.number_of_edges() < 1:
        raise ValueError("manifest graph must contain at least two nodes and one edge")
    density = nx.density(real_graph)
    specs = [GraphSpec("m058_4_layer_9418", real_graph, *choose_path_pair(real_graph))]
    for edge_count in synthetic_edges:
        graph = synthetic_graph(edge_count, density=density, seed=edge_count)
        specs.append(GraphSpec(f"synthetic_{edge_count}", graph, *choose_path_pair(graph)))
    return specs


def _median_ms(operation: Callable[[], Any], runs: int) -> tuple[float, list[float]]:
    timings: list[float] = []
    for _ in range(runs):
        started = time.perf_counter()
        operation()
        timings.append((time.perf_counter() - started) * 1000.0)
    return statistics.median(timings), timings


def _import_optional(module_name: str) -> tuple[Any | None, str | None]:
    try:
        return importlib.import_module(module_name), None
    except Exception as exc:  # pragma: no cover - exercised only when optional deps are absent
        return None, f"{type(exc).__name__}: {exc}"


def _networkx_adapter(spec: GraphSpec) -> LibraryAdapter:
    graph = spec.graph
    operations = {
        "bfs": lambda: list(nx.bfs_tree(graph, spec.source).nodes),
        "pagerank": lambda: nx.pagerank(graph, weight="weight", max_iter=100, tol=1.0e-6),
        "shortest_path": lambda: nx.shortest_path(
            graph, source=spec.source, target=spec.target, weight="weight"
        ),
        "connected_components": lambda: list(nx.weakly_connected_components(graph)),
    }
    return LibraryAdapter("networkx", "ok", graph, None, operations)


def _igraph_adapter(spec: GraphSpec) -> LibraryAdapter:
    igraph, error = _import_optional("igraph")
    if igraph is None:
        return LibraryAdapter("igraph", "skipped", None, error, {})
    try:
        graph = igraph.Graph.from_networkx(spec.graph)
        name_to_index = {name: index for index, name in enumerate(graph.vs["_nx_name"])}
        source = name_to_index[spec.source]
        target = name_to_index[spec.target]
        operations = {
            "bfs": lambda: graph.bfs(source, mode="out"),
            "pagerank": lambda: graph.pagerank(weights="weight"),
            "shortest_path": lambda: graph.get_shortest_paths(
                source, to=target, weights="weight", mode="out"
            ),
            "connected_components": lambda: graph.connected_components(mode="weak"),
        }
        return LibraryAdapter("igraph", "ok", graph, None, operations)
    except Exception as exc:  # pragma: no cover - defensive status surface
        return LibraryAdapter("igraph", "skipped", None, f"{type(exc).__name__}: {exc}", {})


def _rustworkx_adapter(spec: GraphSpec) -> LibraryAdapter:
    rustworkx, error = _import_optional("rustworkx")
    if rustworkx is None:
        return LibraryAdapter("rustworkx", "skipped", None, error, {})
    try:
        graph = rustworkx.PyDiGraph()
        node_to_index = {node: graph.add_node(node) for node in spec.graph.nodes}
        for source, target, data in spec.graph.edges(data=True):
            graph.add_edge(
                node_to_index[source], node_to_index[target], float(data.get("weight", 1.0))
            )
        source = node_to_index[spec.source]
        target = node_to_index[spec.target]
        operations = {
            "bfs": lambda: rustworkx.bfs_successors(graph, source),
            "pagerank": lambda: rustworkx.pagerank(graph, weight_fn=lambda weight: weight),
            "shortest_path": lambda: rustworkx.digraph_dijkstra_shortest_path_lengths(
                graph,
                source,
                lambda weight: weight,
                goal=target,
            ),
            "connected_components": lambda: rustworkx.weakly_connected_components(graph),
        }
        return LibraryAdapter("rustworkx", "ok", graph, None, operations)
    except Exception as exc:  # pragma: no cover - defensive status surface
        return LibraryAdapter("rustworkx", "skipped", None, f"{type(exc).__name__}: {exc}", {})


def build_adapter(library: str, spec: GraphSpec) -> LibraryAdapter:
    """Convert a NetworkX graph to the requested benchmark library."""

    if library == "networkx":
        return _networkx_adapter(spec)
    if library == "igraph":
        return _igraph_adapter(spec)
    if library == "rustworkx":
        return _rustworkx_adapter(spec)
    raise ValueError(f"unknown benchmark library: {library}")


def benchmark_specs(
    specs: Iterable[GraphSpec], *, runs: int = RUNS_PER_OPERATION
) -> dict[str, Any]:
    """Run all benchmark cells and return a JSON-serializable report."""

    results: list[dict[str, Any]] = []
    graph_summaries: list[dict[str, Any]] = []
    for spec in specs:
        graph_summaries.append(
            {
                "name": spec.name,
                "nodes": spec.node_count,
                "edges": spec.edge_count,
                "source": str(spec.source),
                "target": str(spec.target),
            }
        )
        for library in LIBRARIES:
            adapter = build_adapter(library, spec)
            for algorithm in ALGORITHMS:
                row: dict[str, Any] = {
                    "graph": spec.name,
                    "nodes": spec.node_count,
                    "edges": spec.edge_count,
                    "library": library,
                    "algorithm": algorithm,
                    "runs": runs,
                    "status": adapter.status,
                    "latency_ms": None,
                    "samples_ms": [],
                    "error": adapter.error,
                }
                if adapter.status == "ok":
                    try:
                        latency, samples = _median_ms(adapter.operations[algorithm], runs)
                        row.update(
                            {
                                "status": "ok",
                                "latency_ms": round(latency, 3),
                                "samples_ms": [round(sample, 3) for sample in samples],
                                "error": None,
                            }
                        )
                    except Exception as exc:  # pragma: no cover - defensive per-cell status
                        row.update({"status": "error", "error": f"{type(exc).__name__}: {exc}"})
                results.append(row)
    report = {
        "metadata": {
            "milestone": "M061-0fib2i",
            "slice": "S01",
            "runs_per_operation": runs,
            "loopback_host": LOOPBACK_HOST,
            "idempotent": True,
            "benchmark_scope": "research_only_no_runtime_integration",
        },
        "safety_defaults": SAFETY_DEFAULTS,
        "safety_statements": list(SAFETY_STATEMENTS),
        "graphs": graph_summaries,
        "libraries": list(LIBRARIES),
        "algorithms": list(ALGORITHMS),
        "results": results,
        "comparison_table": comparison_table(results),
        "speedups": speedup_table(results),
    }
    return report


def comparison_table(results: Sequence[dict[str, Any]]) -> list[dict[str, Any]]:
    """Pivot benchmark results into one row per graph/library."""

    rows: list[dict[str, Any]] = []
    by_key: dict[tuple[str, str], dict[str, Any]] = {}
    for result in results:
        key = (result["graph"], result["library"])
        row = by_key.setdefault(
            key,
            {
                "graph": result["graph"],
                "library": result["library"],
                "nodes": result["nodes"],
                "edges": result["edges"],
            },
        )
        value: float | str | None = (
            result["latency_ms"] if result["status"] == "ok" else result["status"]
        )
        row[result["algorithm"]] = value
    for graph_name in sorted({result["graph"] for result in results}):
        for library in LIBRARIES:
            rows.append(by_key[(graph_name, library)])
    return rows


def speedup_table(results: Sequence[dict[str, Any]]) -> list[dict[str, Any]]:
    """Compute per-cell speedups against NetworkX median latency."""

    networkx_latencies = {
        (result["graph"], result["algorithm"]): result["latency_ms"]
        for result in results
        if result["library"] == "networkx" and result["status"] == "ok" and result["latency_ms"]
    }
    rows: list[dict[str, Any]] = []
    for result in results:
        baseline = networkx_latencies.get((result["graph"], result["algorithm"]))
        latency = result["latency_ms"]
        speedup = None
        if baseline and latency and result["status"] == "ok":
            speedup = round(float(baseline) / float(latency), 3)
        rows.append(
            {
                "graph": result["graph"],
                "library": result["library"],
                "algorithm": result["algorithm"],
                "speedup_vs_networkx": speedup,
            }
        )
    return rows


def render_markdown(report: dict[str, Any]) -> str:
    """Render a concise benchmark report for ADR-016 input."""

    lines = [
        "# M060c S01 Graph Library Benchmark",
        "",
        "Research-only benchmark for igraph and rustworkx against the existing NetworkX baseline.",
        "",
        f"- Runs per operation: {report['metadata']['runs_per_operation']}",
        f"- Loopback host for any local-only checks: `{report['metadata']['loopback_host']}`",
        "- Runtime integration: none; this is ADR-016 evidence only.",
        "",
        "## Safety defaults",
        "",
    ]
    for statement in report["safety_statements"]:
        lines.append(f"- {statement}")
    lines.extend(
        [
            "",
            "## Latency table (median ms)",
            "",
            "| Graph | Library | Nodes | Edges | BFS | PageRank | Shortest path | Connected components |",
            "|---|---:|---:|---:|---:|---:|---:|---:|",
        ]
    )
    for row in report["comparison_table"]:
        lines.append(
            "| {graph} | {library} | {nodes} | {edges} | {bfs} | {pagerank} | {shortest_path} | {connected_components} |".format(
                **row
            )
        )
    lines.extend(
        [
            "",
            "## Speedup vs NetworkX",
            "",
            "| Graph | Library | Algorithm | Speedup |",
            "|---|---:|---:|---:|",
        ]
    )
    for row in report["speedups"]:
        if row["library"] == "networkx":
            continue
        speedup = row["speedup_vs_networkx"]
        speedup_text = "n/a" if speedup is None else f"{speedup}x"
        lines.append(f"| {row['graph']} | {row['library']} | {row['algorithm']} | {speedup_text} |")
    lines.append("")
    return "\n".join(lines)


def write_report(report: dict[str, Any], output_dir: Path) -> tuple[Path, Path]:
    """Write idempotent JSON and Markdown artifacts."""

    output_dir.mkdir(parents=True, exist_ok=True)
    json_path = output_dir / "benchmark.json"
    md_path = output_dir / "benchmark.md"
    json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    md_path.write_text(render_markdown(report), encoding="utf-8")
    return json_path, md_path


def run_benchmark(
    *,
    manifest_path: Path = DEFAULT_MANIFEST,
    output_dir: Path = DEFAULT_OUTPUT_DIR,
    synthetic_edges: Sequence[int] = (10_000, 100_000),
    runs: int = RUNS_PER_OPERATION,
) -> dict[str, Any]:
    """Run the benchmark and write output artifacts."""

    specs = build_graph_specs(manifest_path, synthetic_edges=synthetic_edges)
    report = benchmark_specs(specs, runs=runs)
    json_path, md_path = write_report(report, output_dir)
    report["metadata"]["json_path"] = str(json_path)
    report["metadata"]["markdown_path"] = str(md_path)
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--synthetic-edges", type=int, nargs="*", default=[10_000, 100_000])
    parser.add_argument("--runs", type=int, default=RUNS_PER_OPERATION)
    args = parser.parse_args()
    report = run_benchmark(
        manifest_path=args.manifest,
        output_dir=args.output_dir,
        synthetic_edges=tuple(args.synthetic_edges),
        runs=args.runs,
    )
    ok_cells = sum(1 for result in report["results"] if result["status"] == "ok")
    skipped_cells = sum(1 for result in report["results"] if result["status"] == "skipped")
    print(
        "benchmark report: "
        f"graphs={len(report['graphs'])} libraries={len(report['libraries'])} "
        f"algorithms={len(report['algorithms'])} ok_cells={ok_cells} skipped_cells={skipped_cells} "
        f"output={report['metadata']['json_path']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
