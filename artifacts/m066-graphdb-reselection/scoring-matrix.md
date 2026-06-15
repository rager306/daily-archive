# M066 S01 GraphDB Re-selection Scoring Matrix

## Scope and safety
This matrix re-evaluates FalkorDB, LadybugDB, Neo4j, HelixDB, and Apache AGE with 18 criteria: the 12 M063 criteria plus six advanced checks for concurrent writes, GRAFBLAS graph algorithms, UDF support, ACID transactions, multi-process safety, and advanced-feature documentation.

The benchmark is offline by default. Network access, production import, graph writes, vendor-source mutation, and real DB connections are disabled. Production graph import is not authorized.

## 18-criteria cross-candidate table

| Criterion | FalkorDB | LadybugDB | Neo4j | HelixDB | Apache AGE |
|---|---:|---:|---:|---:|---:|
| 1. Native vector support | 5 | 5 | 4 | 5 | 3 |
| 2. Python client maturity | 4 | 4 | 5 | 2 | 5 |
| 3. Graph query performance | 4 | 4 | 5 | 4 | 3 |
| 4. Hybrid graph-vector capability | 5 | 5 | 4 | 5 | 4 |
| 5. Migration cost from NetworkX | 3 | 5 | 3 | 3 | 3 |
| 6. Operational complexity, inverted | 3 | 4 | 2 | 3 | 2 |
| 7. License fit | 4 | 4 | 3 | 4 | 4 |
| 8. Community size and activity | 3 | 4 | 5 | 4 | 4 |
| 9. Production readiness | 4 | 3 | 5 | 2 | 3 |
| 10. NetworkX compatibility | 3 | 5 | 3 | 2 | 3 |
| 11. Documentation quality | 4 | 3 | 5 | 3 | 3 |
| 12. Deployment ease | 4 | 4 | 3 | 3 | 2 |
| 13. Concurrent write semantics | 4 | 2 | 5 | 3 | 5 |
| 14. GRAFBLAS graph algorithms | 5 | 1 | 4 | 1 | 1 |
| 15. UDF support | 2 | 3 | 5 | 2 | 5 |
| 16. ACID transactions | 3 | 2 | 5 | 3 | 5 |
| 17. Multi-process safety | 4 | 2 | 5 | 3 | 5 |
| 18. Documentation for advanced features | 4 | 2 | 5 | 2 | 4 |
| **Total score** | **68/90** | **62/90** | **76/90** | **54/90** | **64/90** |
| **Advanced score** | **22/30** | **12/30** | **29/30** | **14/30** | **25/30** |
| **M066 rank** | **#2** | #4 | **#1** | #5 | **#3** |

## Top-3 candidates

### #1 Neo4j — 76/90
Best M066 production candidate after advanced criteria: mature ACID transactions, UDF/procedure support, documented multi-client safety, and strong graph algorithm ecosystem outweigh heavier operations.

### #2 FalkorDB — 68/90
Strong graph-vector fit with server-side serialized writes and GraphBLAS lineage, but weaker UDF and full transaction depth than Neo4j or PostgreSQL-backed AGE.

### #3 Apache AGE — 64/90
Best consolidation option if PostgreSQL becomes the dominant architecture constraint; advanced write/transaction/UDF scores improve its ranking despite weaker native graph-vector ergonomics.

## M063 vs M066 comparison

| Candidate | M063 score | M063 rank | M066 score | M066 rank | Score delta note |
|---|---:|---:|---:|---:|---|
| FalkorDB | 35/45 | #2 | 68/90 | #2 | +33 after advanced criteria; advanced=22/30 |
| LadybugDB | 39/45 | #1 | 62/90 | #4 | +23 after advanced criteria; advanced=12/30 |
| Neo4j | 34/45 | #3 | 76/90 | #1 | +42 after advanced criteria; advanced=29/30 |
| HelixDB | 30/45 | #4 | 54/90 | #5 | +24 after advanced criteria; advanced=14/30 |
| Apache AGE | 28/45 | #5 | 64/90 | #3 | +36 after advanced criteria; advanced=25/30 |

## Winner identification
**Winner: Neo4j (76/90).** Neo4j overtakes the M063 LadybugDB choice because the advanced criteria heavily weight production concurrent writes, ACID transactions, UDFs, multi-process safety, and documentation depth.

## Concurrent write results summary

| Candidate | Final counter | Lost writes | Success rate | Contention events | Avg wait ms | P95 wait ms |
|---|---:|---:|---:|---:|---:|---:|
| FalkorDB | 300 | 0 | 1.0000 | 1 | 0.000117 | 0.000150 |
| LadybugDB | 101 | 199 | 0.3367 | 0 | 0.000000 | 0.000000 |
| Neo4j | 300 | 0 | 1.0000 | 0 | 0.000134 | 0.000170 |
| HelixDB | 300 | 0 | 1.0000 | 10 | 0.000118 | 0.000169 |
| Apache AGE | 300 | 0 | 1.0000 | 1 | 0.000114 | 0.000140 |
