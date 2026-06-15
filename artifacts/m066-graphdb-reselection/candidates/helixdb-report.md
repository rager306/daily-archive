# HelixDB Candidate Report

## 0. One-line summary
Interesting graph-vector system for future review, but current Python maturity, advanced documentation, and GraphBLAS/UDF evidence are not enough to win M066.

**Total score:** 54/90  
**M066 rank:** #5  
**M063 baseline:** 30/45

## 1. Native vector support
Score: **5/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for HelixDB.

## 2. Python client maturity
Score: **2/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for HelixDB.

## 3. Graph query performance
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for HelixDB.

## 4. Hybrid graph-vector capability
Score: **5/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for HelixDB.

## 5. Migration cost from NetworkX
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for HelixDB.

## 6. Operational complexity, inverted
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for HelixDB.

## 7. License fit
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for HelixDB.

## 8. Community size and activity
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for HelixDB.

## 9. Production readiness
Score: **2/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for HelixDB.

## 10. NetworkX compatibility
Score: **2/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for HelixDB.

## 11. Documentation quality
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for HelixDB.

## 12. Deployment ease
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for HelixDB.

## 13. Concurrent write semantics
Score: **3/5**.
Optimistic serialized harness succeeds, but production semantics need live-server proof.

## 14. GRAFBLAS graph algorithms
Score: **1/5**.
No first-class GRAFBLAS support is credited.

## 15. UDF support
Score: **2/5**.
Extension surface is not mature enough for high score.

## 16. ACID transactions
Score: **3/5**.
Partial transaction confidence only; live durability proof is still needed.

## 17. Multi-process safety
Score: **3/5**.
Credited as plausible client/server safety, not yet proven for daily-archive ingestion.

## 18. Documentation for advanced features
Score: **2/5**.
Optimistic serialized harness succeeds, but production semantics need live-server proof.

## Advanced features section
- Concurrent writes: Optimistic serialized harness succeeds, but production semantics need live-server proof.
- GRAFBLAS: No first-class GRAFBLAS support is credited.
- UDF support: Extension surface is not mature enough for high score.
- ACID transactions: Partial transaction confidence only; live durability proof is still needed.
- Multi-process safety: Credited as plausible client/server safety, not yet proven for daily-archive ingestion.

## Concurrent write benchmark
| Metric | Value |
|---|---:|
| Writers | 3 |
| Writes per writer | 100 |
| Attempted writes | 300 |
| Successful write calls | 300 |
| Final counter | 300 |
| Lost writes | 0 |
| Transaction success rate | 1.0000 |
| Write contention events | 10 |
| Avg lock wait ms | 0.000118 |
| P95 lock wait ms | 0.000169 |

## Pros
- Strong graph-vector product direction for agentic KG workloads.
- Rust implementation may become attractive for performance-sensitive paths.

## Cons
- Python integration and production history are less mature than Neo4j, FalkorDB, or AGE.
- No credited GRAFBLAS support and weak UDF evidence.

## Deployment notes
Keep as a watch-list candidate; do not use for production import until live concurrency and recovery evidence exists.

Safety note: production graph import is not authorized; real DB connections are disabled by default.
