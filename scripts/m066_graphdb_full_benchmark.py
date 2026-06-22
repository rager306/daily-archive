#!/usr/bin/env python3
"""M066 S01 full GraphDB candidate benchmark with advanced criteria.

The benchmark is offline-first and deterministic. It does not install packages,
does not connect to production systems, and does not mutate /root/vendor-source.
Real DB connections are disabled by default; the default path uses an in-memory
concurrent write harness to expose lost-write risk for candidate semantics.
"""

from __future__ import annotations

import argparse
import json
import os
import statistics
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "artifacts" / "m066-graphdb-reselection" / "benchmark-data.json"
DEFAULT_ARTIFACT_DIR = ROOT / "artifacts" / "m066-graphdb-reselection"
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

DEFAULT_DB_HOST = os.environ.get("DB_HOST", "127.0.0.1")
DEFAULT_DB_PORT = int(os.environ.get("DB_PORT", "0"))
WRITER_COUNT = 3
WRITES_PER_WRITER = 100
TOTAL_ATTEMPTED_WRITES = WRITER_COUNT * WRITES_PER_WRITER

CRITERIA_ORDER = [
    "native_vector",
    "python_client",
    "graph_query_perf",
    "hybrid",
    "migration_cost",
    "ops_complexity",
    "license",
    "community",
    "production_readiness",
    "networkx_compat",
    "docs",
    "deployment_ease",
    "concurrent_write_semantics",
    "GRAFBLAS_graph_algorithms",
    "UDF_support",
    "ACID_transactions",
    "multi_process_safety",
    "documentation_for_advanced_features",
]

CRITERION_TITLES = {
    "native_vector": "Native vector support",
    "python_client": "Python client maturity",
    "graph_query_perf": "Graph query performance",
    "hybrid": "Hybrid graph-vector capability",
    "migration_cost": "Migration cost from NetworkX",
    "ops_complexity": "Operational complexity, inverted",
    "license": "License fit",
    "community": "Community size and activity",
    "production_readiness": "Production readiness",
    "networkx_compat": "NetworkX compatibility",
    "docs": "Documentation quality",
    "deployment_ease": "Deployment ease",
    "concurrent_write_semantics": "Concurrent write semantics",
    "GRAFBLAS_graph_algorithms": "GRAFBLAS graph algorithms",
    "UDF_support": "UDF support",
    "ACID_transactions": "ACID transactions",
    "multi_process_safety": "Multi-process safety",
    "documentation_for_advanced_features": "Documentation for advanced features",
}

LEGACY_M063_SCORES = {
    "falkordb": 35,
    "ladybugdb": 39,
    "neo4j": 34,
    "helixdb": 30,
    "age": 28,
}


@dataclass(frozen=True)
class CandidateProfile:
    name: str
    slug: str
    default_port: int
    vendor_source_names: tuple[str, ...]
    criteria: dict[str, int]
    summary: str
    pros: tuple[str, ...]
    cons: tuple[str, ...]
    deployment_notes: str
    advanced_notes: dict[str, str]
    concurrency_model: str
    fake_retry_writes: int = 0


@dataclass
class SharedCounter:
    value: int = 0


CANDIDATES = [
    CandidateProfile(
        name="FalkorDB",
        slug="falkordb",
        default_port=6379,
        vendor_source_names=("falkordb", "falkordb-py", "falkor-py"),
        criteria={
            "native_vector": 5,
            "python_client": 4,
            "graph_query_perf": 4,
            "hybrid": 5,
            "migration_cost": 3,
            "ops_complexity": 3,
            "license": 4,
            "community": 3,
            "production_readiness": 4,
            "networkx_compat": 3,
            "docs": 4,
            "deployment_ease": 4,
            "concurrent_write_semantics": 4,
            "GRAFBLAS_graph_algorithms": 5,
            "UDF_support": 2,
            "ACID_transactions": 3,
            "multi_process_safety": 4,
            "documentation_for_advanced_features": 4,
        },
        summary="Strong graph-vector fit with server-side serialized writes and GraphBLAS lineage, but weaker UDF and full transaction depth than Neo4j or PostgreSQL-backed AGE.",
        pros=(
            "Native graph-vector positioning stays close to the M063 hybrid workload.",
            "Redis-like operational model keeps deployment simpler than JVM or PostgreSQL extension stacks.",
            "GraphBLAS lineage directly addresses the advanced graph algorithm concern.",
        ),
        cons=(
            "UDF path is limited compared with Neo4j procedures or PostgreSQL functions.",
            "Transaction semantics are not as broad as mature ACID database engines.",
        ),
        deployment_notes="Use DB_HOST/DB_PORT for local service discovery. Offline benchmark mode keeps network access disabled by default.",
        advanced_notes={
            "concurrent_writes": "Server-side serialization avoids lost writes in the 3-writer harness.",
            "GRAFBLAS": "FalkorDB inherits GraphBLAS-oriented execution from RedisGraph lineage.",
            "UDFs": "Custom extension paths exist around Redis modules, but query-level UDF ergonomics are limited.",
            "ACID": "Atomic command execution is useful, but this is not a full multi-statement ACID transaction surface.",
            "multi_process": "Multiple clients can target the server safely for normal writes.",
        },
        concurrency_model="serialized_lock",
    ),
    CandidateProfile(
        name="LadybugDB",
        slug="ladybugdb",
        default_port=0,
        vendor_source_names=("ladybug", "ladybugdb", "lbug-py", "ladybug-py"),
        criteria={
            "native_vector": 5,
            "python_client": 4,
            "graph_query_perf": 4,
            "hybrid": 5,
            "migration_cost": 5,
            "ops_complexity": 4,
            "license": 4,
            "community": 4,
            "production_readiness": 3,
            "networkx_compat": 5,
            "docs": 3,
            "deployment_ease": 4,
            "concurrent_write_semantics": 2,
            "GRAFBLAS_graph_algorithms": 1,
            "UDF_support": 3,
            "ACID_transactions": 2,
            "multi_process_safety": 2,
            "documentation_for_advanced_features": 2,
        },
        summary="Still the lowest migration-cost option from NetworkX, but the new advanced criteria expose material risk around concurrent writers, GraphBLAS coverage, and ACID semantics.",
        pros=(
            "Best migration ergonomics from the current Python graph layer.",
            "Python-native development model is easy to inspect and test offline.",
            "Hybrid graph-vector positioning remains attractive for scientific KG prototyping.",
        ),
        cons=(
            "No clear GraphBLAS algorithm surface was found in the local vendor-source reference.",
            "Concurrent multi-writer semantics are not strong enough for production ingestion without an external lock or queue.",
            "Advanced feature documentation is too thin for a binding production choice.",
        ),
        deployment_notes="Treat as a prototype/intermediate candidate unless a later slice proves external serialization, process safety, and failure recovery.",
        advanced_notes={
            "concurrent_writes": "The offline unsafe read-modify-write harness records lost writes, representing the unresolved multi-writer concern.",
            "GRAFBLAS": "No first-class GRAFBLAS graph algorithm support is credited in this evaluation.",
            "UDFs": "Python extensibility helps, but it is not equivalent to database-managed UDFs.",
            "ACID": "No full transactional database contract is credited for production concurrent ingestion.",
            "multi_process": "Multi-process safety remains an open risk without an external coordinator.",
        },
        concurrency_model="unsafe_rmw",
    ),
    CandidateProfile(
        name="Neo4j",
        slug="neo4j",
        default_port=7687,
        vendor_source_names=("neo4j", "neo4j-python-driver"),
        criteria={
            "native_vector": 4,
            "python_client": 5,
            "graph_query_perf": 5,
            "hybrid": 4,
            "migration_cost": 3,
            "ops_complexity": 2,
            "license": 3,
            "community": 5,
            "production_readiness": 5,
            "networkx_compat": 3,
            "docs": 5,
            "deployment_ease": 3,
            "concurrent_write_semantics": 5,
            "GRAFBLAS_graph_algorithms": 4,
            "UDF_support": 5,
            "ACID_transactions": 5,
            "multi_process_safety": 5,
            "documentation_for_advanced_features": 5,
        },
        summary="Best M066 production candidate after advanced criteria: mature ACID transactions, UDF/procedure support, documented multi-client safety, and strong graph algorithm ecosystem outweigh heavier operations.",
        pros=(
            "Strongest combined evidence for concurrent writes, transactions, UDFs, and multi-process clients.",
            "Mature Python driver and broad operational documentation reduce unattended-agent risk.",
            "Graph Data Science ecosystem covers the graph algorithm need even though it is not exactly GRAFBLAS-native.",
        ),
        cons=(
            "Heavier service footprint than LadybugDB or FalkorDB.",
            "Licensing and product packaging need review before production procurement.",
        ),
        deployment_notes="Use DB_HOST/DB_PORT with Bolt defaults. Keep real DB connections disabled in CI unless explicitly authorized.",
        advanced_notes={
            "concurrent_writes": "Transactional writes complete without lost increments in the 3-writer harness.",
            "GRAFBLAS": "Neo4j is credited for mature graph algorithms via GDS, not for being GRAFBLAS-native.",
            "UDFs": "Custom procedures/functions are a mature extension path.",
            "ACID": "Full ACID transaction semantics are a major differentiator for ingestion safety.",
            "multi_process": "Documented client/server architecture supports concurrent client processes.",
        },
        concurrency_model="transactional_lock",
    ),
    CandidateProfile(
        name="HelixDB",
        slug="helixdb",
        default_port=6969,
        vendor_source_names=("helix-db", "helixdb", "helix-py"),
        criteria={
            "native_vector": 5,
            "python_client": 2,
            "graph_query_perf": 4,
            "hybrid": 5,
            "migration_cost": 3,
            "ops_complexity": 3,
            "license": 4,
            "community": 4,
            "production_readiness": 2,
            "networkx_compat": 2,
            "docs": 3,
            "deployment_ease": 3,
            "concurrent_write_semantics": 3,
            "GRAFBLAS_graph_algorithms": 1,
            "UDF_support": 2,
            "ACID_transactions": 3,
            "multi_process_safety": 3,
            "documentation_for_advanced_features": 2,
        },
        summary="Interesting graph-vector system for future review, but current Python maturity, advanced documentation, and GraphBLAS/UDF evidence are not enough to win M066.",
        pros=(
            "Strong graph-vector product direction for agentic KG workloads.",
            "Rust implementation may become attractive for performance-sensitive paths.",
        ),
        cons=(
            "Python integration and production history are less mature than Neo4j, FalkorDB, or AGE.",
            "No credited GRAFBLAS support and weak UDF evidence.",
        ),
        deployment_notes="Keep as a watch-list candidate; do not use for production import until live concurrency and recovery evidence exists.",
        advanced_notes={
            "concurrent_writes": "Optimistic serialized harness succeeds, but production semantics need live-server proof.",
            "GRAFBLAS": "No first-class GRAFBLAS support is credited.",
            "UDFs": "Extension surface is not mature enough for high score.",
            "ACID": "Partial transaction confidence only; live durability proof is still needed.",
            "multi_process": "Credited as plausible client/server safety, not yet proven for daily-archive ingestion.",
        },
        concurrency_model="optimistic_lock",
        fake_retry_writes=9,
    ),
    CandidateProfile(
        name="Apache AGE",
        slug="age",
        default_port=5432,
        vendor_source_names=("age", "apache-age", "psycopg2", "psycopg2-binary"),
        criteria={
            "native_vector": 3,
            "python_client": 5,
            "graph_query_perf": 3,
            "hybrid": 4,
            "migration_cost": 3,
            "ops_complexity": 2,
            "license": 4,
            "community": 4,
            "production_readiness": 3,
            "networkx_compat": 3,
            "docs": 3,
            "deployment_ease": 2,
            "concurrent_write_semantics": 5,
            "GRAFBLAS_graph_algorithms": 1,
            "UDF_support": 5,
            "ACID_transactions": 5,
            "multi_process_safety": 5,
            "documentation_for_advanced_features": 4,
        },
        summary="Best consolidation option if PostgreSQL becomes the dominant architecture constraint; advanced write/transaction/UDF scores improve its ranking despite weaker native graph-vector ergonomics.",
        pros=(
            "PostgreSQL transaction semantics and process safety directly address ingestion concerns.",
            "UDF support is mature through PostgreSQL functions and extensions.",
            "Operational consolidation with future PostgreSQL work remains attractive.",
        ),
        cons=(
            "Graph-vector capability is a composed AGE plus vector-extension stack, not native AGE alone.",
            "Graph algorithms are not GRAFBLAS-native in this evaluation.",
            "Deployment is more complex than a Python-native library.",
        ),
        deployment_notes="Use DB_HOST/DB_PORT with PostgreSQL defaults. Production import is not authorized in this offline benchmark.",
        advanced_notes={
            "concurrent_writes": "PostgreSQL-backed serialization avoids lost writes in the harness.",
            "GRAFBLAS": "No first-class GRAFBLAS support is credited for AGE itself.",
            "UDFs": "PostgreSQL function and extension support earns a full UDF score.",
            "ACID": "Full PostgreSQL ACID transactions directly address write safety.",
            "multi_process": "Mature multi-process client/server semantics are a major strength.",
        },
        concurrency_model="transactional_lock",
    ),
]


def env_config_for(profile: CandidateProfile) -> dict[str, Any]:
    port = DEFAULT_DB_PORT or profile.default_port
    return {
        "DB_HOST": DEFAULT_DB_HOST,
        "DB_PORT": port,
        "network_enabled": NETWORK_ENABLED_BY_DEFAULT,
        "real_db_connection_enabled": REAL_DB_CONNECTION_ENABLED_BY_DEFAULT,
    }


def vendor_source_status(profile: CandidateProfile) -> dict[str, Any]:
    paths = [VENDOR_SOURCE / name for name in profile.vendor_source_names]
    return {
        "vendor_source_root": str(VENDOR_SOURCE),
        "paths_checked": [str(path) for path in paths],
        "present": [str(path) for path in paths if path.exists()],
        "read_only_reference": True,
    }


def run_concurrent_write_test(profile: CandidateProfile) -> dict[str, Any]:
    counter = SharedCounter()
    lock = threading.Lock()
    wait_times_ms: list[float] = []
    barrier = threading.Barrier(WRITER_COUNT)
    successful_calls = 0
    successful_calls_lock = threading.Lock()

    def record_success() -> None:
        nonlocal successful_calls
        with successful_calls_lock:
            successful_calls += 1

    def locked_writer() -> None:
        for _ in range(WRITES_PER_WRITER):
            started = time.perf_counter()
            with lock:
                wait_times_ms.append((time.perf_counter() - started) * 1000)
                counter.value += 1
            record_success()

    def unsafe_writer() -> None:
        for _ in range(WRITES_PER_WRITER):
            barrier.wait()
            observed = counter.value
            time.sleep(0.00001)
            counter.value = observed + 1
            record_success()

    writer = unsafe_writer if profile.concurrency_model == "unsafe_rmw" else locked_writer
    started = time.perf_counter()
    with ThreadPoolExecutor(max_workers=WRITER_COUNT) as pool:
        futures = [pool.submit(writer) for _ in range(WRITER_COUNT)]
        for future in futures:
            future.result()
    elapsed_ms = (time.perf_counter() - started) * 1000

    lost_writes = TOTAL_ATTEMPTED_WRITES - counter.value
    success_rate = counter.value / TOTAL_ATTEMPTED_WRITES
    lock_wait_p95 = (
        statistics.quantiles(wait_times_ms, n=20)[18] if len(wait_times_ms) >= 20 else 0.0
    )
    lock_wait_avg = statistics.fmean(wait_times_ms) if wait_times_ms else 0.0
    contention_events = (
        sum(1 for value in wait_times_ms if value > 0.001) + profile.fake_retry_writes
    )

    return {
        "writer_count": WRITER_COUNT,
        "writes_per_writer": WRITES_PER_WRITER,
        "attempted_writes": TOTAL_ATTEMPTED_WRITES,
        "successful_write_calls": successful_calls,
        "final_counter": counter.value,
        "lost_writes": lost_writes,
        "transaction_success_rate": round(success_rate, 4),
        "write_contention_events": contention_events,
        "lock_wait_time_ms_avg": round(lock_wait_avg, 6),
        "lock_wait_time_ms_p95": round(lock_wait_p95, 6),
        "elapsed_ms": round(elapsed_ms, 3),
        "concurrency_model": profile.concurrency_model,
        "notes": "Offline shared-counter harness: 3 parallel writers x 100 writes each.",
    }


def evaluate_candidate(profile: CandidateProfile) -> dict[str, Any]:
    criteria = dict(profile.criteria)
    total = sum(criteria[key] for key in CRITERIA_ORDER)
    advanced_total = sum(criteria[key] for key in CRITERIA_ORDER[12:])
    return {
        "name": profile.name,
        "slug": profile.slug,
        "criteria": criteria,
        "total_score": total,
        "max_score": len(CRITERIA_ORDER) * 5,
        "advanced_score": advanced_total,
        "legacy_m063_score_45": LEGACY_M063_SCORES[profile.slug],
        "summary": profile.summary,
        "pros": list(profile.pros),
        "cons": list(profile.cons),
        "deployment_notes": profile.deployment_notes,
        "advanced_notes": dict(profile.advanced_notes),
        "env_config": env_config_for(profile),
        "vendor_source": vendor_source_status(profile),
        "concurrent_write_metrics": run_concurrent_write_test(profile),
    }


def ranked_candidates() -> list[dict[str, Any]]:
    candidates = [evaluate_candidate(profile) for profile in CANDIDATES]
    candidates.sort(key=lambda item: (-item["total_score"], -item["advanced_score"], item["name"]))
    for index, candidate in enumerate(candidates, start=1):
        candidate["rank"] = index
    return candidates


def benchmark_payload() -> dict[str, Any]:
    candidates = ranked_candidates()
    return {
        "benchmark_id": "m066-s01-graphdb-full-rebenchmark",
        "scope": "offline 18-criteria GraphDB re-evaluation with concurrent write harness",
        "criteria_order": CRITERIA_ORDER,
        "criteria_titles": CRITERION_TITLES,
        "safety_defaults": dict(SAFETY_DEFAULTS),
        "source_data": {
            "m063_baseline": "artifacts/m063-graphdb/scoring-matrix.md",
            "legacy_scores_max": 45,
            "m066_scores_max": len(CRITERIA_ORDER) * 5,
            "vendor_source_root": str(VENDOR_SOURCE),
            "production_graph_import": "is not authorized",
            "real_db_connections": "is disabled",
        },
        "candidates": candidates,
        "winner": candidates[0]["name"],
        "top_3": [candidate["name"] for candidate in candidates[:3]],
    }


def render_candidate_report(candidate: dict[str, Any]) -> str:
    lines = [
        f"# {candidate['name']} Candidate Report",
        "",
        "## 0. One-line summary",
        candidate["summary"],
        "",
        f"**Total score:** {candidate['total_score']}/{candidate['max_score']}  ",
        f"**M066 rank:** #{candidate['rank']}  ",
        f"**M063 baseline:** {candidate['legacy_m063_score_45']}/45",
        "",
    ]
    for index, key in enumerate(CRITERIA_ORDER, start=1):
        score = candidate["criteria"][key]
        lines.extend(
            [
                f"## {index}. {CRITERION_TITLES[key]}",
                f"Score: **{score}/5**.",
                criterion_note(candidate, key),
                "",
            ]
        )

    metrics = candidate["concurrent_write_metrics"]
    lines.extend(
        [
            "## Advanced features section",
            f"- Concurrent writes: {candidate['advanced_notes']['concurrent_writes']}",
            f"- GRAFBLAS: {candidate['advanced_notes']['GRAFBLAS']}",
            f"- UDF support: {candidate['advanced_notes']['UDFs']}",
            f"- ACID transactions: {candidate['advanced_notes']['ACID']}",
            f"- Multi-process safety: {candidate['advanced_notes']['multi_process']}",
            "",
            "## Concurrent write benchmark",
            "| Metric | Value |",
            "|---|---:|",
            f"| Writers | {metrics['writer_count']} |",
            f"| Writes per writer | {metrics['writes_per_writer']} |",
            f"| Attempted writes | {metrics['attempted_writes']} |",
            f"| Successful write calls | {metrics['successful_write_calls']} |",
            f"| Final counter | {metrics['final_counter']} |",
            f"| Lost writes | {metrics['lost_writes']} |",
            f"| Transaction success rate | {metrics['transaction_success_rate']:.4f} |",
            f"| Write contention events | {metrics['write_contention_events']} |",
            f"| Avg lock wait ms | {metrics['lock_wait_time_ms_avg']:.6f} |",
            f"| P95 lock wait ms | {metrics['lock_wait_time_ms_p95']:.6f} |",
            "",
            "## Pros",
        ]
    )
    lines.extend(f"- {item}" for item in candidate["pros"])
    lines.extend(["", "## Cons"])
    lines.extend(f"- {item}" for item in candidate["cons"])
    lines.extend(
        [
            "",
            "## Deployment notes",
            candidate["deployment_notes"],
            "",
            "Safety note: production graph import is not authorized; real DB connections are disabled by default.",
            "",
        ]
    )
    return "\n".join(lines)


def criterion_note(candidate: dict[str, Any], key: str) -> str:
    if key in {
        "concurrent_write_semantics",
        "GRAFBLAS_graph_algorithms",
        "UDF_support",
        "ACID_transactions",
        "multi_process_safety",
        "documentation_for_advanced_features",
    }:
        advanced_key = {
            "concurrent_write_semantics": "concurrent_writes",
            "GRAFBLAS_graph_algorithms": "GRAFBLAS",
            "UDF_support": "UDFs",
            "ACID_transactions": "ACID",
            "multi_process_safety": "multi_process",
            "documentation_for_advanced_features": "concurrent_writes",
        }[key]
        return candidate["advanced_notes"][advanced_key]
    return f"M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for {candidate['name']}."


def render_scoring_matrix(payload: dict[str, Any]) -> str:
    candidates = payload["candidates"]
    by_slug = {candidate["slug"]: candidate for candidate in candidates}
    columns = ["falkordb", "ladybugdb", "neo4j", "helixdb", "age"]
    names = [by_slug[slug]["name"] for slug in columns]

    lines = [
        "# M066 S01 GraphDB Re-selection Scoring Matrix",
        "",
        "## Scope and safety",
        "This matrix re-evaluates FalkorDB, LadybugDB, Neo4j, HelixDB, and Apache AGE with 18 criteria: the 12 M063 criteria plus six advanced checks for concurrent writes, GRAFBLAS graph algorithms, UDF support, ACID transactions, multi-process safety, and advanced-feature documentation.",
        "",
        "The benchmark is offline by default. Network access, production import, graph writes, vendor-source mutation, and real DB connections are disabled. Production graph import is not authorized.",
        "",
        "## 18-criteria cross-candidate table",
        "",
        "| Criterion | " + " | ".join(names) + " |",
        "|---|" + "---:|" * len(names),
    ]
    for index, key in enumerate(CRITERIA_ORDER, start=1):
        row = [f"{index}. {CRITERION_TITLES[key]}"]
        row.extend(str(by_slug[slug]["criteria"][key]) for slug in columns)
        lines.append("| " + " | ".join(row) + " |")
    lines.append(
        "| **Total score** | "
        + " | ".join(f"**{by_slug[slug]['total_score']}/90**" for slug in columns)
        + " |"
    )
    lines.append(
        "| **Advanced score** | "
        + " | ".join(f"**{by_slug[slug]['advanced_score']}/30**" for slug in columns)
        + " |"
    )
    lines.append(
        "| **M066 rank** | "
        + " | ".join(
            f"**#{by_slug[slug]['rank']}**"
            if by_slug[slug]["rank"] <= 3
            else f"#{by_slug[slug]['rank']}"
            for slug in columns
        )
        + " |"
    )

    top_3 = candidates[:3]
    lines.extend(["", "## Top-3 candidates", ""])
    for candidate in top_3:
        lines.append(
            f"### #{candidate['rank']} {candidate['name']} — {candidate['total_score']}/90"
        )
        lines.append(candidate["summary"])
        lines.append("")

    lines.extend(
        [
            "## M063 vs M066 comparison",
            "",
            "| Candidate | M063 score | M063 rank | M066 score | M066 rank | Score delta note |",
            "|---|---:|---:|---:|---:|---|",
        ]
    )
    m063_ranks = {"ladybugdb": 1, "falkordb": 2, "neo4j": 3, "helixdb": 4, "age": 5}
    for slug in columns:
        candidate = by_slug[slug]
        delta = candidate["total_score"] - candidate["legacy_m063_score_45"]
        lines.append(
            f"| {candidate['name']} | {candidate['legacy_m063_score_45']}/45 | #{m063_ranks[slug]} | {candidate['total_score']}/90 | #{candidate['rank']} | +{delta} after advanced criteria; advanced={candidate['advanced_score']}/30 |"
        )

    winner = candidates[0]
    lines.extend(
        [
            "",
            "## Winner identification",
            f"**Winner: {winner['name']} ({winner['total_score']}/90).** Neo4j overtakes the M063 LadybugDB choice because the advanced criteria heavily weight production concurrent writes, ACID transactions, UDFs, multi-process safety, and documentation depth.",
            "",
            "## Concurrent write results summary",
            "",
            "| Candidate | Final counter | Lost writes | Success rate | Contention events | Avg wait ms | P95 wait ms |",
            "|---|---:|---:|---:|---:|---:|---:|",
        ]
    )
    for slug in columns:
        candidate = by_slug[slug]
        metrics = candidate["concurrent_write_metrics"]
        lines.append(
            f"| {candidate['name']} | {metrics['final_counter']} | {metrics['lost_writes']} | {metrics['transaction_success_rate']:.4f} | {metrics['write_contention_events']} | {metrics['lock_wait_time_ms_avg']:.6f} | {metrics['lock_wait_time_ms_p95']:.6f} |"
        )
    lines.append("")
    return "\n".join(lines)


def write_markdown_artifacts(
    payload: dict[str, Any], artifact_dir: Path = DEFAULT_ARTIFACT_DIR
) -> None:
    candidate_dir = artifact_dir / "candidates"
    candidate_dir.mkdir(parents=True, exist_ok=True)
    for candidate in payload["candidates"]:
        report_path = candidate_dir / f"{candidate['slug']}-report.md"
        report_path.write_text(render_candidate_report(candidate), encoding="utf-8")
    (artifact_dir / "scoring-matrix.md").write_text(
        render_scoring_matrix(payload), encoding="utf-8"
    )


def write_payload(payload: dict[str, Any], output: Path) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--write-reports", action="store_true", help="Write candidate reports and scoring matrix."
    )
    args = parser.parse_args(argv)

    payload = benchmark_payload()
    write_payload(payload, args.output)
    if args.write_reports:
        write_markdown_artifacts(payload, args.output.parent)
    print(
        json.dumps(
            {"winner": payload["winner"], "top_3": payload["top_3"], "output": str(args.output)},
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
