# M063 S01 GraphDB Scoring Matrix

## Scope and safety

This matrix compares five GraphDB candidates for M063: FalkorDB, LadybugDB, Neo4j, HelixDB, and Apache AGE. Benchmark data comes from `scripts/m063_graphdb_benchmark.py`, which is offline by default: network access, production import, graph writes, vendor-source mutation, and real DB connections are disabled. The default host in source/config examples is `127.0.0.1`.

The benchmark exercised a deterministic M062-shaped workload: 3,000 nodes, 9,000 edges, and five query shapes (`citation_lookup`, `table_similarity`, `figure_similarity`, `judge_lookup`, `vector_search`). It does **not** claim live server performance.

## 12-criteria cross-candidate table

| Criterion | FalkorDB | LadybugDB | Neo4j | HelixDB | Apache AGE |
|---|---:|---:|---:|---:|---:|
| 1. Native vector support | 5 | 5 | 4 | 5 | 3 |
| 2. Python client maturity | 4 | 4 | 5 | 2 | 5 |
| 3. Graph query performance at 9k edges | 4 | 4 | 5 | 4 | 3 |
| 4. Hybrid graph-vector capability | 5 | 5 | 4 | 5 | 4 |
| 5. Migration cost from NetworkX | 3 | 5 | 3 | 3 | 3 |
| 6. Operational complexity, inverted | 3 | 4 | 2 | 3 | 2 |
| 7. License | MIT client | MIT | Apache-2.0/Python-2.0 driver; product licensing mixed | Apache-2.0 repo; MIT/Apache client text | Apache-2.0 AGE; psycopg2 LGPL exception |
| 8. Community size | 63 stars / 9 forks on Python client | 1,290 stars / 98 forks | 1,046 stars / 212 forks on Python driver | 5,139 stars / 274 forks | 4,597 stars / 503 forks |
| 9. Production readiness | 4 | 3 | 5 | 2 | 3 |
| 10. NetworkX compatibility | 3 | 5 | 3 | 2 | 3 |
| 11. Documentation quality | 4 | 3 | 5 | 3 | 3 |
| 12. Deployment ease | 4 | 4 | 3 | 3 | 2 |
| **Numeric total** | **35/45** | **39/45** | **34/45** | **30/45** | **28/45** |
| **Rank** | **#2** | **#1** | **#3** | #4 | #5 |

## Benchmark evidence summary

| Candidate | Empirical S01 mode | Client import available | Vendored Python client | Load ms | p50 ms | p95 ms | p99 ms |
|---|---|---:|---:|---:|---:|---:|---:|
| FalkorDB | offline in-memory harness | false | false | 0.4848 | 0.0137 | 1.0164 | 1.0563 |
| LadybugDB | offline in-memory harness | true | false | 0.2103 | 0.0136 | 1.0669 | 1.0971 |
| Neo4j | offline in-memory harness | false | false | 0.2795 | 0.0162 | 1.1589 | 1.2731 |
| HelixDB | offline in-memory harness | false | false | 0.2410 | 0.0125 | 1.0594 | 1.1095 |
| Apache AGE | offline in-memory harness | false | false | 0.2771 | 0.0132 | 1.1020 | 1.1086 |

## Top-3 candidates

### 1. LadybugDB — best migration fit
LadybugDB scores highest because it preserves the current Python/NetworkX mental model, has native vector direction, and is already importable in this environment. The main risk is production readiness; S02 should verify persistence, concurrency, backup, and failure-mode behavior before ADR-020 binds to it.

### 2. FalkorDB — best native graph-vector service fit
FalkorDB is the strongest standalone graph-vector service candidate. It has official Python client metadata and native vector support, with lower operational complexity than Neo4j. The main S02 gap is that the Python client is not vendored under `/root/vendor-source/` yet; only `/root/vendor-source/falkordb` source exists.

### 3. Neo4j — safest mature fallback
Neo4j is the maturity and operations documentation leader. It should remain the conservative fallback if production readiness outweighs migration simplicity. The tradeoff is operational weight and adapter work from NetworkX.

## Deferred from S01

- Live server benchmarks are deferred because real DB connections and network use are disabled by default.
- Vendoring missing Python clients is deferred to M063+.
- ADR-020 selection is S02 scope, not S01 scope.
