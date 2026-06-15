---
id: M067-oqsavh
title: "M067 GraphDB re-selection for self-hosted daily-archive"
status: complete
provides:
  - corrected FalkorDB license analysis for self-hosted daily-archive
  - updated GraphDB scoring matrix with FalkorDB at 70/90
  - distribution model assumption for self-hosted research operation
  - binding ADR-022 selecting FalkorDB for production GraphDB
  - ADR-021 and ADR-020 supersession evidence
  - Russian REPORT plus milestone SUMMARY and VALIDATION closeout artifacts
key_decisions:
  - ADR-022 binds daily-archive to FalkorDB as the self-hosted production GraphDB target
  - ADR-022 supersedes ADR-021 and ADR-020 for the current production GraphDB choice
  - SSPLv1 is acceptable for the current self-hosted distribution model
  - SaaS or hosted-service distribution requires commercial FalkorDB licensing or migration to Apache AGE
patterns_established:
  - GraphDB choices must state the distribution model before interpreting licenses
  - Re-scoring must preserve prior benchmark evidence and only change corrected criteria
  - Superseding ADRs must amend both directly and transitively superseded decisions
  - Closeout tests must assert safety posture, trajectory status, and codebase-memory mirror sync
observability_surfaces:
  - artifacts/m066-graphdb-reselection/scoring-matrix.md
  - artifacts/m066-graphdb-reselection/distribution-model.md
  - artifacts/m066-graphdb-reselection/REPORT.md
  - doc/adr/ADR-022-graphdb-reselection-self-hosted.md
  - .codebase-memory/adr.md
  - .codebase-memory/governance-graph.json
---

# M067 Summary: GraphDB re-selection for self-hosted daily-archive

## One-line Outcome

M067 corrected the FalkorDB license analysis, selected **FalkorDB** as the self-hosted production GraphDB target at **70/90**, and closed the ADR chain by superseding ADR-021 and ADR-020 with ADR-022.

## Context

M066 selected Neo4j after an 18-criteria GraphDB re-evaluation. That result was technically strong, but later review found that FalkorDB had been treated as if it carried AGPLv3 constraints. M067 re-opened only the self-hosted selection question and preserved the M066 benchmark harness, candidate set, and safety posture.

M067 explicitly documented the distribution model: daily-archive is a self-hosted research project, not a hosted GraphDB service for third parties. Under that model, FalkorDB's SSPLv1 license is viable for the current project shape.

## Slice Outcomes

### S01: Re-score FalkorDB with SSPLv1 + distribution model

S01 updated the scoring matrix and distribution model. FalkorDB's license-fit score moved from 3/5 to 4/5, raising the total score from 68/90 to **70/90**.

The self-hosted ranking became:

1. FalkorDB: 70/90
2. Apache AGE: 64/90
3. LadybugDB: 62/90

S01 also preserved the benchmark boundaries: production graph import is not authorized, graph writes are disabled, network access is controlled by explicit overrides, real database connections are disabled by default, and vendor-source mutation is disabled.

### S02: ADR-022 binding FalkorDB + supersede

S02 created `doc/adr/ADR-022-graphdb-reselection-self-hosted.md` using the M034 decision template style. ADR-022 is accepted, binding, and revisable only through a later binding ADR with stronger evidence or a changed distribution model.

ADR-022 supersedes both prior GraphDB selection decisions:

- ADR-021: Neo4j selection from M066.
- ADR-020: LadybugDB selection from M063/M065.

ADR-021 and ADR-020 were amended so future readers see the current binding decision without reconstructing milestone history.

### S03: REPORT + closeout

S03 emitted the Russian M067 closeout report, milestone SUMMARY, milestone VALIDATION, codebase-memory mirror sync, and regression tests.

`artifacts/m066-graphdb-reselection/REPORT.md` now presents M067 in eight sections: summary, context, S01 re-score, S02 ADR, top self-hosted candidates, FalkorDB rationale, tradeoffs, and migration/lesson follow-ups.

## Safety and Boundaries

M067 is a decision and closeout milestone. It does not perform production migration.

Safety boundaries preserved:

- production graph import is not authorized;
- production graph writes are disabled;
- network access remains controlled by explicit overrides;
- real database connections are disabled for the benchmark harness;
- vendor-source mutation is disabled.

The five safety defaults remain false unless a later milestone explicitly and safely overrides them.

## Migration Follow-up

Downstream work should treat FalkorDB as the selected self-hosted target, but still needs implementation proof:

1. map article, citation, table, figure, judge/evidence, and queue/work-state entities into FalkorDB nodes and relationships;
2. rewrite the smallest useful NetworkX flow into Cypher-backed FalkorDB operations;
3. prove transaction behavior and retry/idempotency boundaries around ingestion;
4. retain Redis for queue and coordination state where it remains the simpler primitive;
5. keep Apache AGE as the fallback if daily-archive becomes SaaS or a hosted service for third parties.

## Verification

S03 verification is anchored by `tests/test_m067_s03.py`, plus inherited S01/S02 regressions. The closeout test suite checks:

- REPORT exists and has exactly sections 0-7;
- SUMMARY and VALIDATION artifacts exist;
- FalkorDB 70/90 is documented as the self-hosted choice;
- ADR-022 is referenced from report, summary, validation, and codebase-memory;
- ADR-021 and ADR-020 supersession remains documented;
- codebase-memory mirrors all canonical ADRs, including ADR-022;
- M050/M067 S01/S02 regression evidence remains intact;
- M045 stays on_track and M044 stays ok.

## Final State

M067 closes with FalkorDB as the current binding production GraphDB target for self-hosted daily-archive. ADR-022 is the authoritative decision. M064 and later GraphDB implementation milestones should start from FalkorDB, not Neo4j or LadybugDB, unless a later accepted binding ADR changes the distribution model or evidence base.
