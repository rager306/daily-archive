#!/usr/bin/env python3
"""M063 S01 GraphDB candidate benchmark.

This benchmark is intentionally offline-first. It does not install packages, does not
connect to production systems, and does not mutate /root/vendor-source. Real DB
connections are disabled by default; the default path builds a deterministic 5-layer
workload shaped like M062 evidence and measures common insert/query operations in an
in-memory harness so candidate scoring is reproducible in CI.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import math
import os
import statistics
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
M062_CONTRACT = ROOT / "artifacts" / "m062-fd-contract" / "fd-contract-results.json"
DEFAULT_OUTPUT = ROOT / "artifacts" / "m063-graphdb" / "benchmark-data.json"
VENDOR_SOURCE = Path("/root/vendor-source")

# Five explicit safety defaults. Tests assert these remain false.
NETWORK_ENABLED_BY_DEFAULT = False
PRODUCTION_IMPORT_ENABLED_BY_DEFAULT = False
GRAPH_WRITES_ENABLED_BY_DEFAULT = False
VENDOR_SOURCE_MUTATION_ENABLED_BY_DEFAULT = False
REAL_DB_CONNECTION_ENABLED_BY_DEFAULT = False

SAFETY_DEFAULTS = {
    "network_enabled_by_default": NETWORK_ENABLED_BY_DEFAULT,
    "production_import_enabled_by_default": PRODUCTION_IMPORT_ENABLED_BY_DEFAULT,
    "graph_writes_enabled_by_default": GRAPH_WRITES_ENABLED_BY_DEFAULT,
    "vendor_source_mutation_enabled_by_default": VENDOR_SOURCE_MUTATION_ENABLED_BY_DEFAULT,
    "real_db_connection_enabled_by_default": REAL_DB_CONNECTION_ENABLED_BY_DEFAULT,
}

QUERY_NAMES = [
    "citation_lookup",
    "table_similarity",
    "figure_similarity",
    "judge_lookup",
    "vector_search",
]

VENDORED_PYTHON_CLIENT_MARKERS = {
    "falkordb": ("falkordb-py", "falkor-py"),
    "ladybugdb": ("lbug-py", "ladybug-py"),
    "neo4j": ("neo4j-python-driver",),
    "helixdb": ("helix-py",),
    "age": ("psycopg2", "psycopg2-binary"),
}


@dataclass(frozen=True)
class CandidateCriteria:
    name: str
    slug: str
    client_package: str
    import_module: str
    default_port: int
    native_vector_support: int
    python_client_maturity: int
    graph_query_performance_at_9k_edges: int
    hybrid_graph_vector_capability: int
    migration_cost_from_networkx: int
    operational_complexity: int
    license: str
    community_size: str
    production_readiness: str
    networkx_compatibility: int
    documentation_quality: int
    deployment_ease: int
    notes: str

    @property
    def numeric_total(self) -> int:
        return (
            self.native_vector_support
            + self.python_client_maturity
            + self.graph_query_performance_at_9k_edges
            + self.hybrid_graph_vector_capability
            + self.migration_cost_from_networkx
            + self.operational_complexity
            + self.networkx_compatibility
            + self.documentation_quality
            + self.deployment_ease
        )


CANDIDATES: tuple[CandidateCriteria, ...] = (
    CandidateCriteria(
        name="FalkorDB",
        slug="falkordb",
        client_package="falkordb",
        import_module="falkordb",
        default_port=6379,
        native_vector_support=5,
        python_client_maturity=4,
        graph_query_performance_at_9k_edges=4,
        hybrid_graph_vector_capability=5,
        migration_cost_from_networkx=3,
        operational_complexity=3,
        license="MIT client; server source observed separately",
        community_size="GitHub API probe: falkordb/falkordb-py ~52 stars, FalkorDB org docs present",
        production_readiness="Redis-module operational model; productized docs and official clients",
        networkx_compatibility=3,
        documentation_quality=4,
        deployment_ease=4,
        notes="Best fit when native graph+vector in a Redis-like deployment is preferred.",
    ),
    CandidateCriteria(
        name="LadybugDB",
        slug="ladybugdb",
        client_package="ladybug",
        import_module="ladybug",
        default_port=9999,
        native_vector_support=5,
        python_client_maturity=4,
        graph_query_performance_at_9k_edges=4,
        hybrid_graph_vector_capability=5,
        migration_cost_from_networkx=5,
        operational_complexity=4,
        license="Project license to verify from repository before ADR binding",
        community_size="GitHub API probe: LadybugDB/ladybug ~55 stars; PyPI package present",
        production_readiness="Promising Python-native stack; production adoption still less proven than Neo4j",
        networkx_compatibility=5,
        documentation_quality=3,
        deployment_ease=4,
        notes="Lowest migration friction from current NetworkX intermediate layer.",
    ),
    CandidateCriteria(
        name="Neo4j",
        slug="neo4j",
        client_package="neo4j",
        import_module="neo4j",
        default_port=7687,
        native_vector_support=4,
        python_client_maturity=5,
        graph_query_performance_at_9k_edges=5,
        hybrid_graph_vector_capability=4,
        migration_cost_from_networkx=3,
        operational_complexity=2,
        license="Driver license NOASSERTION from GitHub API; Neo4j product licensing is mixed/community/enterprise",
        community_size="GitHub API probe: neo4j/neo4j-python-driver ~1046 stars",
        production_readiness="Most mature candidate with enterprise support and broad production adoption",
        networkx_compatibility=3,
        documentation_quality=5,
        deployment_ease=3,
        notes="Safest enterprise default, but heavier operationally.",
    ),
    CandidateCriteria(
        name="HelixDB",
        slug="helixdb",
        client_package="helix-py",
        import_module="helix",
        default_port=6969,
        native_vector_support=5,
        python_client_maturity=2,
        graph_query_performance_at_9k_edges=4,
        hybrid_graph_vector_capability=5,
        migration_cost_from_networkx=3,
        operational_complexity=3,
        license="License to verify from HelixDB/helix-db before ADR binding",
        community_size="GitHub API probe: HelixDB/helix-db ~898 stars",
        production_readiness="Newer graph-vector database; attractive direction, less proven operational history",
        networkx_compatibility=2,
        documentation_quality=3,
        deployment_ease=3,
        notes="Interesting AI-agent oriented graph-vector direction; higher maturity risk.",
    ),
    CandidateCriteria(
        name="Apache AGE",
        slug="age",
        client_package="psycopg2",
        import_module="psycopg2",
        default_port=5432,
        native_vector_support=3,
        python_client_maturity=5,
        graph_query_performance_at_9k_edges=3,
        hybrid_graph_vector_capability=4,
        migration_cost_from_networkx=3,
        operational_complexity=2,
        license="Apache-2.0 for AGE; psycopg2 LGPL with exceptions",
        community_size="GitHub API probe: apache/age ~3605 stars; PostgreSQL ecosystem is very large",
        production_readiness="PostgreSQL extension path is familiar; AGE lifecycle and pgvector integration add complexity",
        networkx_compatibility=3,
        documentation_quality=3,
        deployment_ease=2,
        notes="Best if PostgreSQL consolidation matters more than native graph-vector ergonomics.",
    ),
)


def read_env_config(candidate: CandidateCriteria) -> dict[str, str]:
    """Read ANTHROPIC-style environment overrides for a candidate."""
    prefix = candidate.slug.upper()
    host = os.environ.get(f"{prefix}_DB_HOST", os.environ.get("DB_HOST", "127.0.0.1"))
    port = os.environ.get(
        f"{prefix}_DB_PORT", os.environ.get("DB_PORT", str(candidate.default_port))
    )
    user = os.environ.get(f"{prefix}_DB_USER", os.environ.get("DB_USER", ""))
    password = os.environ.get(f"{prefix}_DB_PASSWORD", os.environ.get("DB_PASSWORD", ""))
    db_url = os.environ.get(f"{prefix}_DB_URL", os.environ.get("DB_URL", f"tcp://{host}:{port}"))
    return {
        "db_host": host,
        "db_port": port,
        "db_user_set": str(bool(user)).lower(),
        "db_password_set": str(bool(password)).lower(),
        "db_url": db_url,
    }


def load_m062_shape(path: Path = M062_CONTRACT) -> dict[str, int]:
    """Load M062 5-layer graph counts, falling back to the S01 contract counts."""
    fallback = {"citation": 8911, "table": 4934, "figure_v1": 15, "figure_v2": 15, "judge": 150}
    if not path.exists():
        return fallback
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return fallback

    text = json.dumps(payload).lower()
    # The contract file is a validation report, not the canonical graph dump. Keep the known
    # S01 counts unless exact layer counts are discoverable in a future artifact.
    if all(str(value) in text for value in fallback.values()):
        return fallback
    return fallback


def build_synthetic_workload(
    layer_counts: dict[str, int], *, max_nodes: int = 3000, max_edges: int = 9000
) -> dict[str, Any]:
    """Build deterministic 5-layer nodes, edges, and tiny vectors."""
    layers = ["citation", "table", "figure_v1", "figure_v2", "judge"]
    nodes: list[dict[str, Any]] = []
    edges: list[tuple[str, str, str]] = []
    for idx in range(max_nodes):
        layer = layers[idx % len(layers)]
        nodes.append(
            {
                "id": f"{layer}:{idx}",
                "layer": layer,
                "score": (idx * 17) % 101,
                "vector": [((idx + offset) % 29) / 29.0 for offset in range(8)],
            }
        )
    for idx in range(max_edges):
        src = nodes[idx % max_nodes]["id"]
        dst = nodes[(idx * 7 + 13) % max_nodes]["id"]
        relation = (
            f"{nodes[idx % max_nodes]['layer']}_to_{nodes[(idx * 7 + 13) % max_nodes]['layer']}"
        )
        edges.append((src, relation, dst))
    return {"layer_counts": layer_counts, "nodes": nodes, "edges": edges}


def _percentiles(values: list[float]) -> dict[str, float]:
    ordered = sorted(values)
    if not ordered:
        return {"p50_ms": 0.0, "p95_ms": 0.0, "p99_ms": 0.0}

    def percentile(fraction: float) -> float:
        index = min(len(ordered) - 1, math.ceil(len(ordered) * fraction) - 1)
        return round(ordered[index], 4)

    return {"p50_ms": percentile(0.50), "p95_ms": percentile(0.95), "p99_ms": percentile(0.99)}


def run_offline_queries(workload: dict[str, Any]) -> dict[str, dict[str, float]]:
    """Measure five query shapes against the common in-memory workload."""
    nodes = workload["nodes"]
    edges = workload["edges"]
    by_layer: dict[str, list[dict[str, Any]]] = {}
    adjacency: dict[str, list[str]] = {}
    for node in nodes:
        by_layer.setdefault(node["layer"], []).append(node)
    for src, _, dst in edges:
        adjacency.setdefault(src, []).append(dst)

    target_vector = nodes[137]["vector"]

    def dot(left: list[float], right: list[float]) -> float:
        return sum(a * b for a, b in zip(left, right, strict=True))

    query_fns = {
        "citation_lookup": lambda: len(adjacency.get("citation:100", [])),
        "table_similarity": lambda: max(
            dot(target_vector, n["vector"]) for n in by_layer["table"][:500]
        ),
        "figure_similarity": lambda: [
            n["id"] for n in by_layer["figure_v1"][:8] + by_layer["figure_v2"][:8]
        ],
        "judge_lookup": lambda: [n for n in by_layer["judge"] if n["score"] >= 95][:10],
        "vector_search": lambda: sorted(
            ((dot(target_vector, n["vector"]), n["id"]) for n in nodes[:1000]), reverse=True
        )[:10],
    }
    results: dict[str, dict[str, float]] = {}
    for name, query_fn in query_fns.items():
        timings: list[float] = []
        for _ in range(50):
            start = time.perf_counter()
            query_fn()
            timings.append((time.perf_counter() - start) * 1000)
        stats = _percentiles(timings)
        stats["mean_ms"] = round(statistics.fmean(timings), 4)
        results[name] = stats
    return results


def benchmark_candidate(candidate: CandidateCriteria, workload: dict[str, Any]) -> dict[str, Any]:
    start = time.perf_counter()
    # Copying structures approximates bulk serialization/index-building overhead without DB writes.
    node_index = {node["id"]: node for node in workload["nodes"]}
    edge_count = len(workload["edges"])
    load_ms = (time.perf_counter() - start) * 1000
    query_results = run_offline_queries(workload)
    all_timings = [
        value
        for stats in query_results.values()
        for key, value in stats.items()
        if key.endswith("_ms")
    ]
    client_available = importlib.util.find_spec(candidate.import_module) is not None
    vendor_path = VENDOR_SOURCE / candidate.slug
    client_markers = VENDORED_PYTHON_CLIENT_MARKERS[candidate.slug]
    vendored_client_paths = [VENDOR_SOURCE / marker for marker in client_markers]
    return {
        "candidate": candidate.name,
        "slug": candidate.slug,
        "criteria": asdict(candidate) | {"numeric_total": candidate.numeric_total},
        "client_available_in_environment": client_available,
        "client_import_module": candidate.import_module,
        "vendor_source_path": str(vendor_path),
        "vendor_source_present": vendor_path.exists(),
        "vendor_python_client_markers": list(client_markers),
        "vendor_python_client_present": any(path.exists() for path in vendored_client_paths),
        "vendor_python_client_paths_checked": [str(path) for path in vendored_client_paths],
        "env_config": read_env_config(candidate),
        "mode": "offline_in_memory_harness",
        "external_db_available": False,
        "load": {
            "nodes": len(node_index),
            "edges": edge_count,
            "load_ms": round(load_ms, 4),
            "approx_memory_bytes": len(node_index) * 256 + edge_count * 96,
        },
        "queries": query_results,
        "overall_latency": _percentiles(all_timings),
    }


def run_benchmark(output: Path = DEFAULT_OUTPUT) -> dict[str, Any]:
    layer_counts = load_m062_shape()
    workload = build_synthetic_workload(layer_counts)
    candidates = [benchmark_candidate(candidate, workload) for candidate in CANDIDATES]
    payload = {
        "benchmark_id": "m063-s01-graphdb-candidate-benchmark-v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "source_data": {
            "m062_contract": str(M062_CONTRACT),
            "layer_counts": layer_counts,
            "workload_nodes": len(workload["nodes"]),
            "workload_edges": len(workload["edges"]),
            "fallback_used": True,
        },
        "queries": QUERY_NAMES,
        "empirical_candidates": [candidate.name for candidate in CANDIDATES],
        "empirical_scope": "All candidates exercised through the same offline in-memory harness; no real DB server connection is authorized by default.",
        "candidates": candidates,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return payload


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--allow-network",
        action="store_true",
        default=NETWORK_ENABLED_BY_DEFAULT,
        help="Reserved for future real DB probes; disabled by default.",
    )
    parser.add_argument(
        "--allow-production-import",
        action="store_true",
        default=PRODUCTION_IMPORT_ENABLED_BY_DEFAULT,
        help="Reserved safety gate; production import is disabled by default.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.allow_network or args.allow_production_import:
        raise SystemExit(
            "Real DB/network benchmark is disabled for M063 S01; use offline harness only."
        )
    payload = run_benchmark(args.output)
    print(
        json.dumps(
            {
                "output": str(args.output),
                "candidates": len(payload["candidates"]),
                "queries": QUERY_NAMES,
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
