# FalkorDB Candidate Report

## 0. One-line summary
Strong graph-vector fit with server-side serialized writes and GraphBLAS lineage, but weaker UDF and full transaction depth than Neo4j or PostgreSQL-backed AGE.

**Total score:** 68/90  
**M066 rank:** #2  
**M063 baseline:** 35/45

## 1. Native vector support
Score: **5/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for FalkorDB.

## 2. Python client maturity
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for FalkorDB.

## 3. Graph query performance
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for FalkorDB.

## 4. Hybrid graph-vector capability
Score: **5/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for FalkorDB.

## 5. Migration cost from NetworkX
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for FalkorDB.

## 6. Operational complexity, inverted
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for FalkorDB.

## 7. License fit
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for FalkorDB.

## 8. Community size and activity
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for FalkorDB.

## 9. Production readiness
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for FalkorDB.

## 10. NetworkX compatibility
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for FalkorDB.

## 11. Documentation quality
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for FalkorDB.

## 12. Deployment ease
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for FalkorDB.

## 13. Concurrent write semantics
Score: **4/5**.
Server-side serialization avoids lost writes in the 3-writer harness.

## 14. GRAFBLAS graph algorithms
Score: **5/5**.
FalkorDB inherits GraphBLAS-oriented execution from RedisGraph lineage.

## 15. UDF support
Score: **2/5**.
Custom extension paths exist around Redis modules, but query-level UDF ergonomics are limited.

## 16. ACID transactions
Score: **3/5**.
Atomic command execution is useful, but this is not a full multi-statement ACID transaction surface.

## 17. Multi-process safety
Score: **4/5**.
Multiple clients can target the server safely for normal writes.

## 18. Documentation for advanced features
Score: **4/5**.
Server-side serialization avoids lost writes in the 3-writer harness.

## Advanced features section
- Concurrent writes: Server-side serialization avoids lost writes in the 3-writer harness.
- GRAFBLAS: FalkorDB inherits GraphBLAS-oriented execution from RedisGraph lineage.
- UDF support: Custom extension paths exist around Redis modules, but query-level UDF ergonomics are limited.
- ACID transactions: Atomic command execution is useful, but this is not a full multi-statement ACID transaction surface.
- Multi-process safety: Multiple clients can target the server safely for normal writes.

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
| Write contention events | 1 |
| Avg lock wait ms | 0.000117 |
| P95 lock wait ms | 0.000150 |

## Pros
- Native graph-vector positioning stays close to the M063 hybrid workload.
- Redis-like operational model keeps deployment simpler than JVM or PostgreSQL extension stacks.
- GraphBLAS lineage directly addresses the advanced graph algorithm concern.

## Cons
- UDF path is limited compared with Neo4j procedures or PostgreSQL functions.
- Transaction semantics are not as broad as mature ACID database engines.

## Deployment notes
Use DB_HOST/DB_PORT for local service discovery. Offline benchmark mode keeps network access disabled by default.

Safety note: production graph import is not authorized; real DB connections are disabled by default.
