# LadybugDB Candidate Report

## 0. One-line summary
Still the lowest migration-cost option from NetworkX, but the new advanced criteria expose material risk around concurrent writers, GraphBLAS coverage, and ACID semantics.

**Total score:** 62/90  
**M066 rank:** #4  
**M063 baseline:** 39/45

## 1. Native vector support
Score: **5/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for LadybugDB.

## 2. Python client maturity
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for LadybugDB.

## 3. Graph query performance
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for LadybugDB.

## 4. Hybrid graph-vector capability
Score: **5/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for LadybugDB.

## 5. Migration cost from NetworkX
Score: **5/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for LadybugDB.

## 6. Operational complexity, inverted
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for LadybugDB.

## 7. License fit
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for LadybugDB.

## 8. Community size and activity
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for LadybugDB.

## 9. Production readiness
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for LadybugDB.

## 10. NetworkX compatibility
Score: **5/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for LadybugDB.

## 11. Documentation quality
Score: **3/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for LadybugDB.

## 12. Deployment ease
Score: **4/5**.
M066 retains the M063 evidence category and rescales it as an explicit 1-5 criterion for LadybugDB.

## 13. Concurrent write semantics
Score: **2/5**.
The offline unsafe read-modify-write harness records lost writes, representing the unresolved multi-writer concern.

## 14. GRAFBLAS graph algorithms
Score: **1/5**.
No first-class GRAFBLAS graph algorithm support is credited in this evaluation.

## 15. UDF support
Score: **3/5**.
Python extensibility helps, but it is not equivalent to database-managed UDFs.

## 16. ACID transactions
Score: **2/5**.
No full transactional database contract is credited for production concurrent ingestion.

## 17. Multi-process safety
Score: **2/5**.
Multi-process safety remains an open risk without an external coordinator.

## 18. Documentation for advanced features
Score: **2/5**.
The offline unsafe read-modify-write harness records lost writes, representing the unresolved multi-writer concern.

## Advanced features section
- Concurrent writes: The offline unsafe read-modify-write harness records lost writes, representing the unresolved multi-writer concern.
- GRAFBLAS: No first-class GRAFBLAS graph algorithm support is credited in this evaluation.
- UDF support: Python extensibility helps, but it is not equivalent to database-managed UDFs.
- ACID transactions: No full transactional database contract is credited for production concurrent ingestion.
- Multi-process safety: Multi-process safety remains an open risk without an external coordinator.

## Concurrent write benchmark
| Metric | Value |
|---|---:|
| Writers | 3 |
| Writes per writer | 100 |
| Attempted writes | 300 |
| Successful write calls | 300 |
| Final counter | 101 |
| Lost writes | 199 |
| Transaction success rate | 0.3367 |
| Write contention events | 0 |
| Avg lock wait ms | 0.000000 |
| P95 lock wait ms | 0.000000 |

## Pros
- Best migration ergonomics from the current Python graph layer.
- Python-native development model is easy to inspect and test offline.
- Hybrid graph-vector positioning remains attractive for scientific KG prototyping.

## Cons
- No clear GraphBLAS algorithm surface was found in the local vendor-source reference.
- Concurrent multi-writer semantics are not strong enough for production ingestion without an external lock or queue.
- Advanced feature documentation is too thin for a binding production choice.

## Deployment notes
Treat as a prototype/intermediate candidate unless a later slice proves external serialization, process safety, and failure recovery.

Safety note: production graph import is not authorized; real DB connections are disabled by default.
