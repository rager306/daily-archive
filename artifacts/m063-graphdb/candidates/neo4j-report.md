# Neo4j Candidate Report

## 0. One-line summary
Neo4j is the safest maturity candidate and the #3 S01 option, but its operational footprint is heavier than LadybugDB or FalkorDB.

## 1. Description
Neo4j is the most established graph database in this candidate set. It offers Cypher, mature tooling, a first-party Python driver, enterprise support, and graph data science/vector capabilities that can cover daily-archive's graph and embedding needs.

## 2. Native vector support
Score: **4/5**. Neo4j supports vector indexes/search in modern versions and has GraphRAG/GDS ecosystem support. The vector path is mature but more product/version-dependent than FalkorDB's graph-vector positioning.

## 3. Python client maturity
Score: **5/5**. Public API probe: `neo4j/neo4j-python-driver` has 1,046 stars, 212 forks, pushed 2026-06-11. PyPI package `neo4j` version 6.2.0 uploaded 2026-05-04 with Apache-2.0 AND Python-2.0 license expression. PyPI JSON does not expose download counts, so download counts were not fabricated.

## 4. Graph query performance
Score: **5/5**. Offline M063 harness used 3,000 nodes and 9,000 edges from the M062-shaped workload. Load: ~0.24 ms. Overall latency: p50 ~0.016 ms, p95 ~1.159 ms, p99 ~1.273 ms. These are offline workload numbers only; real Neo4j server benchmarking is disabled in S01.

## 5. Hybrid graph+vector
Score: **4/5**. Neo4j can support graph and vector operations, especially with recent vector index and GraphRAG/GDS tooling. It is powerful but may require more version-specific setup.

## 6. Migration cost from NetworkX
Score: **3/5**. Migration requires a Cypher adapter, schema/index DDL, and careful batching.

```python
# Sketch only; no production import is authorized in S01.
with driver.session() as session:
    session.run("MERGE (n:Evidence {id: $id}) SET n += $attrs", id=node_id, attrs=attrs)
    session.run("MATCH (a {id:$src}), (b {id:$dst}) MERGE (a)-[:CITES]->(b)", src=src, dst=dst)
```

## 7. Operational complexity
Score: **2/5**. Neo4j introduces a dedicated DB service, schema migrations, heap/page-cache tuning, backups, auth, and potentially enterprise licensing choices. This is the main downside.

## 8. License + community
License: PyPI driver Apache-2.0 AND Python-2.0; GitHub API reported NOASSERTION for the repo license field. Community data: 1,046 stars and 212 forks for the Python driver. No vendored Python client path (`neo4j-python-driver`) was found under `/root/vendor-source/`.

## 9. Production readiness
Score: **5/5**. Strongest production maturity and support story among the five. If risk tolerance prioritizes operational maturity over migration simplicity, Neo4j becomes the conservative choice.

## 10. NetworkX compatibility
Score: **3/5**. No native NetworkX import/export was verified. A serializer can preserve current graph semantics, but the adapter is non-trivial.

## 11. Documentation quality
Score: **5/5**. Neo4j has extensive driver, Cypher, vector, and operations documentation.

## 12. Deployment ease
Score: **3/5**. Docker/local deployment is straightforward, but production deployment is heavier than embedded/Python-native candidates. S01 env defaults use `127.0.0.1`; network and production import are disabled by default.

## 13. Pros
- Best maturity and support profile.
- Mature Python driver.
- Strong Cypher ecosystem and operational documentation.
- Good fallback if S02 prioritizes enterprise reliability.

## 14. Cons
- Heaviest operational complexity.
- Migration from NetworkX requires a robust adapter.
- Licensing/deployment choices need care before ADR-020.

## 15. Total score
Numeric criteria total: **34/45**. Ranked **#3** in S01 scoring.

## 16. References
- `artifacts/m063-graphdb/benchmark-data.json`
- GitHub/PyPI API probe captured in `.gsd/exec/4a782941-c8ac-49d2-a6b1-d1bbfe9d5248.stdout`
- Neo4j Python driver and vector documentation
