# HelixDB Candidate Report

## 0. One-line summary
HelixDB is an attractive graph-vector candidate for AI-agent workloads, but S01 ranks it below the top three because Python maturity and production history are still riskier.

## 1. Description
HelixDB is an open-source graph-vector database built in Rust and positioned for AI/agent graph workloads. It is relevant to M063 because it treats vectors and graph traversal as a combined database problem rather than a bolt-on integration.

## 2. Native vector support
Score: **5/5**. Native graph-vector positioning is the central reason to include HelixDB in the shortlist.

## 3. Python client maturity
Score: **2/5**. Public API probe: `HelixDB/helix-db` has 5,139 stars, 274 forks, Apache-2.0 license, pushed 2026-06-12. PyPI package `helix-py` version 0.2.31 uploaded 2025-11-11 with MIT/Apache dual-license text. PyPI JSON does not expose download counts, so download counts were not fabricated. Import check in the current environment: not available.

## 4. Graph query performance
Score: **4/5**. Offline M063 harness used 3,000 nodes and 9,000 edges from the M062-shaped workload. Load: ~0.56 ms. Overall latency: p50 ~0.013 ms, p95 ~1.059 ms, p99 ~1.110 ms. These numbers measure the common offline workload only, not a live HelixDB server.

## 5. Hybrid graph+vector
Score: **5/5**. Strong conceptual match for hybrid evidence retrieval, especially if future M064+ flows want agent-oriented graph memory and vector lookup together.

## 6. Migration cost from NetworkX
Score: **3/5**. Migration likely needs a custom adapter and query DSL translation.

```python
# Sketch only; real HelixDB writes are disabled in S01.
for node_id, attrs in graph.nodes(data=True):
    helix.insert_node("Evidence", {"id": node_id, **attrs})
for src, dst, attrs in graph.edges(data=True):
    helix.insert_edge(src, dst, "REL", attrs)
```

## 7. Operational complexity
Score: **3/5**. Rust service deployment may be efficient, but operational recipes, backups, and monitoring need validation before use.

## 8. License + community
License: Apache-2.0 repository; PyPI package reports MIT/Apache dual-license text. Community data: 5,139 stars and 274 forks for `HelixDB/helix-db`. No vendored Python client path (`helix-py`) was found under `/root/vendor-source/`.

## 9. Production readiness
Score: **2/5**. Strong momentum and high stars, but production adoption evidence is not as established as Neo4j, and the Python client is newer.

## 10. NetworkX compatibility
Score: **2/5**. No native NetworkX compatibility was verified. A migration adapter would likely be custom.

## 11. Documentation quality
Score: **3/5**. Public documentation and repository material are sufficient for exploration, but S02 would need exact Python, schema, vector, backup, and failure-mode docs.

## 12. Deployment ease
Score: **3/5**. Potentially easy for local use, not yet proven for daily-archive production operations. S01 default host is `127.0.0.1`; network and production import are disabled by default.

## 13. Pros
- Native graph-vector database.
- High public interest.
- Rust implementation may have strong performance potential.
- Good conceptual fit for agent-oriented graph retrieval.

## 14. Cons
- Python client maturity risk.
- No vendored client source.
- Production readiness and operations are not yet proven enough for S01 top-3.

## 15. Total score
Numeric criteria total: **30/45**. Ranked **#4** in S01 scoring.

## 16. References
- `artifacts/m063-graphdb/benchmark-data.json`
- GitHub/PyPI API probe captured in `.gsd/exec/4a782941-c8ac-49d2-a6b1-d1bbfe9d5248.stdout`
- HelixDB repository and package metadata
