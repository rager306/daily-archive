# Neo4j Candidate Report

## 0. One-line summary
Best M066 production candidate after advanced criteria: mature ACID transactions, UDF/procedure support, documented multi-client safety, and strong graph algorithm ecosystem outweigh heavier operations.

**Total score:** 76/90  
**M066 rank:** #1  
**M063 baseline:** 34/45

## 1. Native vector support
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Neo4j.

## 2. Python client maturity
Score: **5/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Neo4j.

## 3. Graph query performance
Score: **5/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Neo4j.

## 4. Hybrid graph-vector capability
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Neo4j.

## 5. Migration cost from NetworkX
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Neo4j.

## 6. Operational complexity, inverted
Score: **2/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Neo4j.

## 7. License fit
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Neo4j.

## 8. Community size and activity
Score: **5/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Neo4j.

## 9. Production readiness
Score: **5/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Neo4j.

## 10. NetworkX compatibility
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Neo4j.

## 11. Documentation quality
Score: **5/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Neo4j.

## 12. Deployment ease
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Neo4j.

## 13. Concurrent write semantics
Score: **5/5**.
Transactional writes complete without lost increments in the 3-writer harness.

## 14. GRAFBLAS graph algorithms
Score: **4/5**.
Neo4j is credited for mature graph algorithms via GDS, not for being GRAFBLAS-native.

## 15. UDF support
Score: **5/5**.
Custom procedures/functions are a mature extension path.

## 16. ACID transactions
Score: **5/5**.
Full ACID transaction semantics are a major differentiator for ingestion safety.

## 17. Multi-process safety
Score: **5/5**.
Documented client/server architecture supports concurrent client processes.

## 18. Documentation for advanced features
Score: **5/5**.
Transactional writes complete without lost increments in the 3-writer harness.

## Advanced features section
- Concurrent writes: Transactional writes complete without lost increments in the 3-writer harness.
- GRAFBLAS: Neo4j is credited for mature graph algorithms via GDS, not for being GRAFBLAS-native.
- UDF support: Custom procedures/functions are a mature extension path.
- ACID transactions: Full ACID transaction semantics are a major differentiator for ingestion safety.
- Multi-process safety: Documented client/server architecture supports concurrent client processes.

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
| Write contention events | 0 |
| Avg lock wait ms | 0.000134 |
| P95 lock wait ms | 0.000170 |

## Pros
- Strongest combined evidence for concurrent writes, transactions, UDFs, and multi-process clients.
- Mature Python driver and broad operational documentation reduce unattended-agent risk.
- Graph Data Science ecosystem covers the graph algorithm need even though it is not exactly GRAFBLAS-native.

## Cons
- Heavier service footprint than LadybugDB or FalkorDB.
- Licensing and product packaging need review before production procurement.

## Deployment notes
Use DB_HOST/DB_PORT with Bolt defaults. Keep real DB connections disabled in CI unless explicitly authorized.

Safety note: production graph import is not authorized; real DB connections are disabled by default.
