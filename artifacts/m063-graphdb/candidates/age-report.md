# Apache AGE Candidate Report

## 0. One-line summary
Apache AGE is the best PostgreSQL-consolidation option, but it ranks fifth for M063 because graph-vector ergonomics and operations are less direct than the top candidates.

## 1. Description
Apache AGE adds graph querying to PostgreSQL. For daily-archive, it is attractive if M066+ PostgreSQL consolidation becomes the dominant architecture constraint. Vector support would normally come through PostgreSQL extensions such as pgvector rather than AGE itself.

## 2. Native vector support
Score: **3/5**. AGE itself is graph-focused. Hybrid graph-vector is possible in PostgreSQL with pgvector, but it is a composed stack rather than native AGE capability.

## 3. Python client maturity
Score: **5/5**. The practical Python path uses PostgreSQL clients such as `psycopg2`. Public API probe: `apache/age` has 4,597 stars, 503 forks, Apache-2.0 license, pushed 2026-06-11. PyPI package `psycopg2` version 2.9.12 uploaded 2026-04-20 with LGPL-with-exceptions license text. PyPI JSON does not expose download counts, so download counts were not fabricated.

## 4. Graph query performance
Score: **3/5**. Offline M063 harness used 3,000 nodes and 9,000 edges from the M062-shaped workload. Load: ~0.27 ms. Overall latency: p50 ~0.013 ms, p95 ~1.102 ms, p99 ~1.109 ms. These are common offline workload numbers, not a live PostgreSQL/AGE benchmark.

## 5. Hybrid graph+vector
Score: **4/5**. PostgreSQL plus AGE plus pgvector can unify graph, relational metadata, and embeddings, but the integration is operationally more complex than a native graph-vector DB.

## 6. Migration cost from NetworkX
Score: **3/5**. Migration requires transforming NetworkX into Cypher-like AGE queries and managing PostgreSQL schemas/extensions.

```python
# Sketch only; production import is disabled in S01.
with conn.cursor() as cur:
    cur.execute("SELECT * FROM cypher('daily_archive', $$ CREATE (:Evidence {id: $id}) $$) AS (v agtype)")
```

## 7. Operational complexity
Score: **2/5**. PostgreSQL is familiar, but AGE + pgvector + graph schema migrations creates a multi-extension operating model.

## 8. License + community
License: Apache-2.0 for AGE; `psycopg2` is LGPL with exceptions. Community data: 4,597 stars and 503 forks for `apache/age`. No vendored Python client path (`psycopg2` or `psycopg2-binary`) was found under `/root/vendor-source/`.

## 9. Production readiness
Score: **3/5**. PostgreSQL itself is production-proven, but AGE as the graph layer and combined pgvector graph workflow require more validation.

## 10. NetworkX compatibility
Score: **3/5**. No native NetworkX import/export was verified. Serialization is straightforward, but query semantics and graph updates need an adapter.

## 11. Documentation quality
Score: **3/5**. AGE and PostgreSQL docs exist, but combined AGE + pgvector + Python graph workflow documentation is less cohesive than Neo4j or FalkorDB.

## 12. Deployment ease
Score: **2/5**. Easy if PostgreSQL is already mandatory; otherwise, extension setup and version compatibility are heavier. S01 default host is `127.0.0.1`; network and production import are disabled by default.

## 13. Pros
- Strong PostgreSQL ecosystem.
- Apache-2.0 AGE license.
- Could align with M066+ PostgreSQL conditional path.
- Mature Python PostgreSQL clients.

## 14. Cons
- Vector support is not native to AGE.
- Multi-extension stack increases operational complexity.
- Lower graph-vector ergonomics for M063 than LadybugDB/FalkorDB/Neo4j.

## 15. Total score
Numeric criteria total: **28/45**. Ranked **#5** in S01 scoring.

## 16. References
- `artifacts/m063-graphdb/benchmark-data.json`
- GitHub/PyPI API probe captured in `.gsd/exec/4a782941-c8ac-49d2-a6b1-d1bbfe9d5248.stdout`
- Apache AGE and psycopg2 package metadata
