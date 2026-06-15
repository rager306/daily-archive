# Apache AGE Candidate Report

## 0. One-line summary
Best consolidation option if PostgreSQL becomes the dominant architecture constraint; advanced write/transaction/UDF scores improve its ranking despite weaker native graph-vector ergonomics.

**Total score:** 64/90  
**M066 rank:** #3  
**M063 baseline:** 28/45

## 1. Native vector support
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Apache AGE.

## 2. Python client maturity
Score: **5/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Apache AGE.

## 3. Graph query performance
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Apache AGE.

## 4. Hybrid graph-vector capability
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Apache AGE.

## 5. Migration cost from NetworkX
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Apache AGE.

## 6. Operational complexity, inverted
Score: **2/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Apache AGE.

## 7. License fit
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Apache AGE.

## 8. Community size and activity
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Apache AGE.

## 9. Production readiness
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Apache AGE.

## 10. NetworkX compatibility
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Apache AGE.

## 11. Documentation quality
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Apache AGE.

## 12. Deployment ease
Score: **2/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for Apache AGE.

## 13. Concurrent write semantics
Score: **5/5**.
PostgreSQL-backed serialization avoids lost writes in the harness.

## 14. GRAFBLAS graph algorithms
Score: **1/5**.
No first-class GRAFBLAS support is credited for AGE itself.

## 15. UDF support
Score: **5/5**.
PostgreSQL function and extension support earns a full UDF score.

## 16. ACID transactions
Score: **5/5**.
Full PostgreSQL ACID transactions directly address write safety.

## 17. Multi-process safety
Score: **5/5**.
Mature multi-process client/server semantics are a major strength.

## 18. Documentation for advanced features
Score: **4/5**.
PostgreSQL-backed serialization avoids lost writes in the harness.

## Advanced features section
- Concurrent writes: PostgreSQL-backed serialization avoids lost writes in the harness.
- GRAFBLAS: No first-class GRAFBLAS support is credited for AGE itself.
- UDF support: PostgreSQL function and extension support earns a full UDF score.
- ACID transactions: Full PostgreSQL ACID transactions directly address write safety.
- Multi-process safety: Mature multi-process client/server semantics are a major strength.

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
| Avg lock wait ms | 0.000114 |
| P95 lock wait ms | 0.000140 |

## Pros
- PostgreSQL transaction semantics and process safety directly address ingestion concerns.
- UDF support is mature through PostgreSQL functions and extensions.
- Operational consolidation with future PostgreSQL work remains attractive.

## Cons
- Graph-vector capability is a composed AGE plus vector-extension stack, not native AGE alone.
- Graph algorithms are not GRAFBLAS-native in this evaluation.
- Deployment is more complex than a Python-native library.

## Deployment notes
Use DB_HOST/DB_PORT with PostgreSQL defaults. Production import is not authorized in this offline benchmark.

Safety note: production graph import is not authorized; real DB connections are disabled by default.
