---
id: M066-7fbv31
title: "M066 GraphDB re-evaluation with advanced criteria"
status: complete
provides:
  - 18-criteria GraphDB benchmark across five candidates
  - candidate reports and scoring matrix for GraphDB re-selection
  - binding ADR-021 selecting Neo4j as production GraphDB target
  - ADR-020 supersession evidence preserving LadybugDB decision history
  - Russian REPORT plus milestone SUMMARY and VALIDATION closeout artifacts
key_decisions:
  - ADR-021 binds daily-archive to Neo4j as the production GraphDB target
  - ADR-021 supersedes ADR-020, replacing LadybugDB with Neo4j after M066 evidence
  - Advanced criteria are required for GraphDB decisions touching production ingestion
  - Production graph import is not authorized by M066 alone
patterns_established:
  - GraphDB decisions must include concurrent write evidence, transaction posture, UDF support, algorithm support, and multi-process safety
  - Superseding ADRs must name the replaced ADR and preserve historical context
  - Benchmark reports must separate decision evidence from production mutation authority
observability_surfaces:
  - M066 scoring matrix
  - Five candidate reports
  - ADR-021 binding decision record
  - REPORT.md synthesis
requirements_validated: []
---

# Milestone Summary: M066-7fbv31

## One-line Summary

M066 re-evaluated GraphDB candidates with advanced production criteria and selected Neo4j at 76/90, superseding ADR-020's LadybugDB selection at 62/90.

## What Changed

M066 converted the GraphDB decision from a 12-criterion M063/M065 selection into an 18-criterion production-readiness evaluation. The milestone kept the original five candidates: FalkorDB, LadybugDB, Neo4j, HelixDB, and Apache AGE.

The new criteria tested the risks that mattered after M063:

- concurrent write behavior;
- GRAFBLAS-class algorithm support;
- UDF/procedure support;
- ACID transaction posture;
- multi-process safety;
- advanced-feature documentation.

Those criteria changed the decision. LadybugDB had led M063 at 39/45, but M066 evidence showed 101 successful concurrent writes out of 300 attempts, 199 lost writes, and only 12/30 on advanced criteria. Neo4j ranked first overall at 76/90 and 29/30 on advanced criteria.

## Slice Delivery

| Slice | Delivered | Evidence |
|---|---|---|
| S01 | Full 18-criteria benchmark with five candidate reports and scoring matrix | `artifacts/m066-graphdb-reselection/scoring-matrix.md`, candidate reports, `tests/test_m066_s01.py` |
| S02 | Binding re-decision ADR | `doc/adr/ADR-021-graphdb-reselection.md`, amended `doc/adr/ADR-020-graphdb-selection.md`, `tests/test_m066_s02.py` |
| S03 | Synthesis and closeout | `artifacts/m066-graphdb-reselection/REPORT.md`, this summary, validation, `tests/test_m066_s03.py` |

## Decision Outcome

ADR-021 is now the binding GraphDB selection for daily-archive. It selects Neo4j as the production GraphDB target and supersedes ADR-020.

Final M066 ranking:

| Rank | Candidate | Score | Outcome |
|---:|---|---:|---|
| 1 | Neo4j | 76/90 | Selected |
| 2 | FalkorDB | 68/90 | Not selected |
| 3 | Apache AGE | 64/90 | Conditional future option |
| 4 | LadybugDB | 62/90 | Superseded |
| 5 | HelixDB | 54/90 | Not selected |

## Why Neo4j Won

Neo4j was not the lowest-operational-cost option. It won because the advanced production risks mattered more than operational simplicity.

Neo4j scored strongly on the requirements that block production ingestion risk:

- concurrent writes: 5/5;
- GRAFBLAS-class graph algorithms: 4/5;
- UDF/procedure support: 5/5;
- ACID transactions: 5/5;
- multi-process safety: 5/5;
- advanced documentation: 5/5.

This gives the next graph migration milestone a clearer path for transactions, retries, idempotency, and operational diagnostics.

## Safety and Boundaries

M066 is a decision and evidence milestone. It does not perform production migration.

Safety boundaries preserved:

- production graph import is not authorized;
- production graph writes are disabled;
- network access remains controlled by explicit overrides;
- real database connections are disabled for the benchmark harness;
- vendor-source mutation is disabled.

The five safety defaults remain false unless a later milestone explicitly and safely overrides them.

## Migration Follow-up

Downstream work should treat Neo4j as the selected target, but still needs implementation proof:

1. map article, citation, table, figure, judge/evidence, and queue/work-state entities into Neo4j nodes and relationships;
2. rewrite NetworkX graph operations into Cypher and Neo4j driver transactions;
3. define idempotency and retry boundaries around write transactions;
4. add operational health and diagnostic surfaces for Neo4j connection and transaction failures;
5. run fixture-based verification before any production graph import is allowed.

## Requirement and Decision Effects

M066 advances the GraphDB selection requirement by replacing the superseded LadybugDB choice with a benchmark-backed Neo4j decision.

Decision effects:

- ADR-020 remains historical M063/M065 evidence.
- ADR-021 is the current binding choice.
- codebase-memory governance mirror was regenerated after ADR-021, with ADR-020 and ADR-021 present in the mirror.

## Verification

S03 verification is captured in `tests/test_m066_s03.py` and the M066 validation artifact.

Expected final checks:

- `uv run pytest tests/test_m066_s03.py -q` passes;
- M045 trajectory remains on_track;
- M044 architecture guardrail remains ok;
- codebase-memory `--check` reports no stale mirror;
- no remote push is performed.

## Deviations

The S03 plan referred to codebase-memory as 17 ADRs, but the regenerated mirror currently contains 21 ADRs through ADR-021. The closeout treats this as an expected growth of canonical ADR history, not a failure.

## Known Limitations

Neo4j has not been deployed or connected to production by this milestone. Operational complexity, license posture, backup/restore, credentials, and service monitoring remain follow-up implementation concerns.

## Follow-ups

- Use Neo4j transactions as the target path for M064 queue/graph work.
- Keep Apache AGE as a conditional future option if PostgreSQL consolidation becomes dominant.
- Keep fd v2 verification upstream of graph ingestion evidence.
- File any production migration work as a separate milestone with explicit safety gates.
