# LadybugDB Candidate Report

## 0. One-line summary
LadybugDB is the highest-scoring S01 candidate because it is Python-native, vector-capable, and has the lowest migration friction from the current NetworkX intermediate layer.

## 1. Description
LadybugDB is a Python graph database package already present in the project dependency set as `ladybug`. It is especially relevant because ADR-015/ADR-016 established NetworkX as the current intermediate graph layer, and LadybugDB appears designed for Python-first graph workflows.

## 2. Native vector support
Score: **5/5**. Project positioning and package metadata support native graph/vector use. This matches daily-archive's table similarity, figure similarity, and future embedding-backed evidence retrieval.

## 3. Python client maturity
Score: **4/5**. Public API probe: `LadybugDB/ladybug` has 1,290 stars, 98 forks, MIT license, pushed 2026-06-12. PyPI package `ladybug` version 0.17.1 uploaded 2026-06-02. PyPI JSON does not expose download counts, so download counts were not fabricated. Import check in the current environment: available.

## 4. Graph query performance
Score: **4/5**. Offline M063 harness used 3,000 nodes and 9,000 edges from the M062-shaped workload. Load: ~0.23 ms. Overall latency: p50 ~0.014 ms, p95 ~1.067 ms, p99 ~1.097 ms. These are common in-memory harness numbers, not a real LadybugDB storage benchmark.

## 5. Hybrid graph+vector
Score: **5/5**. LadybugDB is the strongest candidate for keeping graph traversal and vector retrieval inside a Python-native interface, reducing adapter surface area.

## 6. Migration cost from NetworkX
Score: **5/5**. Expected migration is closest to current code because NetworkX-style nodes/edges can be serialized with minimal conceptual translation.

```python
# Sketch only; production graph import is disabled in S01.
for node_id, attrs in graph.nodes(data=True):
    ladybug_graph.add_node(node_id, **attrs)
for src, dst, attrs in graph.edges(data=True):
    ladybug_graph.add_edge(src, dst, **attrs)
```

## 7. Operational complexity
Score: **4/5**. Python-native deployment keeps the moving-parts count low. S02 must verify persistence, concurrency, backup, and operational observability before binding ADR-020.

## 8. License + community
License: MIT from GitHub/PyPI probes. Community data: 1,290 stars and 98 forks for `LadybugDB/ladybug`. No vendored Python client path (`lbug-py` or `ladybug-py`) was found under `/root/vendor-source/`; the package is installed through project dependencies instead.

## 9. Production readiness
Score: **3/5**. The project is active and popular enough for serious consideration, but production adoption evidence is thinner than Neo4j's. This is the primary risk for choosing it.

## 10. NetworkX compatibility
Score: **5/5**. Best candidate for NetworkX migration because it stays in Python and can preserve node/edge attribute semantics with small adapters.

## 11. Documentation quality
Score: **3/5**. Enough exists for S01 research, but ADR-020 should require exact persistence/vector examples and failure-mode docs before final selection.

## 12. Deployment ease
Score: **4/5**. Likely easiest to embed in the current Python workflow. S01 config still supports env overrides (`DB_HOST`, `DB_PORT`, `DB_USER`, `DB_PASSWORD`) with default host `127.0.0.1`; network and production import are disabled by default.

## 13. Pros
- Highest score in S01.
- Python import is available in the current environment.
- Lowest NetworkX migration friction.
- Native graph-vector direction aligns with M062/M063.

## 14. Cons
- Production-readiness evidence is weaker than Neo4j.
- Python-client vendoring is deferred.
- S02 must verify persistence/concurrency guarantees.

## 15. Total score
Numeric criteria total: **39/45**. Ranked **#1** in S01 scoring.

## 16. References
- `artifacts/m063-graphdb/benchmark-data.json`
- GitHub/PyPI API probe captured in `.gsd/exec/4a782941-c8ac-49d2-a6b1-d1bbfe9d5248.stdout`
- PyPI package metadata for `ladybug`
