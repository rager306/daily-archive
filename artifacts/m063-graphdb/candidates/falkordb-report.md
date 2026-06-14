# FalkorDB Candidate Report

## 0. One-line summary
FalkorDB is a strong top-3 candidate for M063 because it combines Cypher-style graph querying with native vector search in a Redis-like operational shape.

## 1. Description
FalkorDB is a graph database derived from RedisGraph lineage and exposed through official clients, including the `falkordb` Python client. For daily-archive, it fits the hybrid scientific KG target better than pure graph-only stores because vector search is a first-class product capability rather than an external add-on.

## 2. Native vector support
Score: **5/5**. FalkorDB documents vector indexing/search as a native database capability. This directly matches the citation/table/figure/judge hybrid graph-vector workload.

## 3. Python client maturity
Score: **4/5**. Public API probe: `falkordb/falkordb-py` has 63 stars, 9 forks, MIT license, pushed 2026-06-08. PyPI package `falkordb` version 1.6.1 uploaded 2026-04-28. PyPI JSON does not expose download counts, so download counts were not fabricated.

## 4. Graph query performance
Score: **4/5**. Offline M063 harness used 3,000 nodes and 9,000 edges from the M062-shaped workload. Load: ~0.59 ms. Overall latency: p50 ~0.014 ms, p95 ~1.016 ms, p99 ~1.056 ms. These are in-memory harness numbers, not a real FalkorDB server benchmark; real DB benchmarking is disabled for S01.

## 5. Hybrid graph+vector
Score: **5/5**. FalkorDB's core value proposition is graph plus vector retrieval, so it can model citation edges while supporting table/figure similarity lookup without a separate vector database.

## 6. Migration cost from NetworkX
Score: **3/5**. Migration requires translating NetworkX node/edge attributes to Cypher writes and query patterns.

```python
# Sketch only; real DB writes are disabled in S01.
for node_id, attrs in graph.nodes(data=True):
    cypher = "MERGE (n:PaperNode {id: $id}) SET n += $attrs"
for src, dst, attrs in graph.edges(data=True):
    cypher = "MATCH (a {id:$src}), (b {id:$dst}) MERGE (a)-[:REL {kind:$kind}]->(b)"
```

## 7. Operational complexity
Score: **3/5**. Redis-like service operation is lighter than Neo4j Enterprise but still adds a separate daemon, persistence, backups, and versioned query/schema migrations.

## 8. License + community
License: Python client MIT. Community data: GitHub API probe showed 63 stars and 9 forks for `falkordb/falkordb-py`. `/root/vendor-source/falkordb` is present, but no vendored Python client path (`falkordb-py` or `falkor-py`) was found.

## 9. Production readiness
Score: **4/5**. Official docs and clients exist, and the operational model is familiar to Redis operators. It is less universally battle-tested than Neo4j but more purpose-fit for graph-vector than AGE.

## 10. NetworkX compatibility
Score: **3/5**. No native NetworkX import/export was verified. Compatibility is feasible through explicit node/edge serialization.

## 11. Documentation quality
Score: **4/5**. Official docs include client references and vector material. S02 should still verify exact Cypher/vector DDL before ADR-020.

## 12. Deployment ease
Score: **4/5**. A single service on default port 6379 is straightforward. S01 config uses env overrides (`DB_HOST`, `DB_PORT`, `DB_USER`, `DB_PASSWORD`) with default host `127.0.0.1`; network use is disabled by default.

## 13. Pros
- Native graph-vector story.
- Python client is recent and official.
- Lower operational burden than a full Neo4j stack.
- Good fit for M062 five-layer graph plus similarity queries.

## 14. Cons
- Python client source is not vendored yet.
- Requires Cypher translation from NetworkX.
- Production history appears narrower than Neo4j.

## 15. Total score
Numeric criteria total: **35/45**. Ranked **#2** in S01 scoring.

## 16. References
- `artifacts/m063-graphdb/benchmark-data.json`
- GitHub API probe for `falkordb/falkordb-py`, captured in `.gsd/exec/4a782941-c8ac-49d2-a6b1-d1bbfe9d5248.stdout`
- FalkorDB official client docs: https://docs.falkordb.com/getting-started/clients.html
