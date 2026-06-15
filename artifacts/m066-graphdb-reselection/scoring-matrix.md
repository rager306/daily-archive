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
| **Total score** | **70/90** | **62/90** | **76/90** | **54/90** | **64/90** |
| **Advanced score** | **22/30** | **12/30** | **29/30** | **14/30** | **25/30** |
| **M066 rank** | **#2** | #4 | **#1** | #5 | **#3** |
| **M067 self-hosted rank** | **#1** | #3 | #3 by license risk | #4 | **#2** |

## Top candidates under the M067 self-hosted assumption

### Total-score leader: Neo4j — 76/90
Neo4j remains the highest total scorer after advanced criteria, but its AGPLv3 license is viral for self-hosted use too. M066 acknowledged the license risk but did not resolve it for daily-archive's self-hosted distribution model.

### #1 self-hosted candidate: FalkorDB — 70/90
FalkorDB becomes the best self-hosted fit after correcting the license model to SSPLv1 and assuming daily-archive remains a self-hosted research project. SSPLv1 is acceptable for this distribution model while preserving the strong graph-vector fit, server-side serialized writes, and GraphBLAS lineage.

### #2 self-hosted candidate: Apache AGE — 64/90
Apache AGE remains the cleanest permissive fallback if PostgreSQL consolidation or future SaaS distribution becomes the dominant architecture constraint.

### #3 self-hosted candidate: LadybugDB — 62/90
LadybugDB remains license-clean and simple, but M066 advanced criteria exposed weaker multi-process and advanced graph algorithm coverage.

**M067 self-hosted ranking:** FalkorDB 70/90 > Apache AGE 64/90 > LadybugDB 62/90. Neo4j is 76/90 by total score but is not selected for the self-hosted ranking because AGPLv3 remains viral for self-hosted distribution. License-clean candidates remain Apache AGE 64/90, LadybugDB 62/90, and HelixDB 54/90.

## M063 vs M066 comparison

| Candidate | M063 score | M063 rank | M066 score | M066 rank | Score delta note |
|---|---:|---:|---:|---:|---|
| FalkorDB | 35/45 | #2 | 70/90 | #1 self-hosted | +35 after advanced criteria and M067 SSPLv1 correction; advanced=22/30 |
| LadybugDB | 39/45 | #1 | 62/90 | #4 | +23 after advanced criteria; advanced=12/30 |
| Neo4j | 34/45 | #3 | 76/90 | #1 | +42 after advanced criteria; advanced=29/30 |
| HelixDB | 30/45 | #4 | 54/90 | #5 | +24 after advanced criteria; advanced=14/30 |
| Apache AGE | 28/45 | #5 | 64/90 | #3 | +36 after advanced criteria; advanced=25/30 |

## Winner identification
**Total-score winner: Neo4j (76/90).** Neo4j still has the strongest advanced production feature score, but M067 does not select it for self-hosted daily-archive because AGPLv3 remains viral for self-hosted use.

**M067 self-hosted winner: FalkorDB (70/90).** FalkorDB wins under the explicit daily-archive distribution model assumption: self-hosted research KG, single-user operation today, and no hosted third-party FalkorDB service exposure.

## Concurrent write results summary

| Candidate | Final counter | Lost writes | Success rate | Contention events | Avg wait ms | P95 wait ms |
|---|---:|---:|---:|---:|---:|---:|
| FalkorDB | 300 | 0 | 1.0000 | 1 | 0.000117 | 0.000150 |
| LadybugDB | 101 | 199 | 0.3367 | 0 | 0.000000 | 0.000000 |
| Neo4j | 300 | 0 | 1.0000 | 0 | 0.000134 | 0.000170 |
| HelixDB | 300 | 0 | 1.0000 | 10 | 0.000118 | 0.000169 |
| Apache AGE | 300 | 0 | 1.0000 | 1 | 0.000114 | 0.000140 |
