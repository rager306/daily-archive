---
id: M065-u29n4f
title: "M063 GraphDB selection"
status: complete
provides:
  - binding GraphDB selection for daily-archive scientific KG
  - five-candidate GraphDB benchmark and scoring matrix
  - ADR-020 selecting LadybugDB as primary production GraphDB
  - M063 closeout REPORT, SUMMARY, and VALIDATION artifacts
key_decisions:
  - ADR-020 binds daily-archive to LadybugDB as the primary production GraphDB for M063 follow-on work
  - NetworkX remains the intermediate correctness baseline during migration
  - production graph writes remain gated by per-paper atomic DAG evidence
  - FalkorDB and Neo4j remain documented alternatives, not the selected path
patterns_established:
  - offline GraphDB candidate comparison before production substrate selection
  - M034-style binding ADR for GraphDB selection decisions
  - per-paper atomic NetworkX to LadybugDB migration plan
observability_surfaces:
  - M063 scoring matrix
  - five candidate reports
  - ADR-020 migration and acceptance criteria
  - codebase-memory governance mirror
requirement_outcomes:
  - id: GraphDB-selection
    from_status: active
    to_status: validated
    proof: ADR-020 and artifacts/m063-graphdb/scoring-matrix.md select LadybugDB at 39/45
duration: S01-S03
verification_result: passed
completed_at: 2026-06-14
---

# M065-u29n4f: M063 GraphDB selection

**M063 selected LadybugDB as the primary production GraphDB for the daily-archive scientific knowledge graph, backed by five candidate reports, a 12-criteria scoring matrix, and binding ADR-020.**

## What Happened

M063 closed the project’s open GraphDB-selection question. S01 created an offline benchmark harness and evaluated five candidates: LadybugDB, FalkorDB, Neo4j, HelixDB, and Apache AGE. The benchmark used a deterministic M062-shaped workload and produced a scoring matrix that ranked LadybugDB first at 39/45.

S02 converted that evidence into ADR-020. The ADR explicitly selects LadybugDB as the primary production GraphDB and preserves NetworkX as the intermediate correctness baseline. It also records FalkorDB, Neo4j, HelixDB, and Apache AGE as alternatives considered with their tradeoffs.

S03 synthesized the evidence into `artifacts/m063-graphdb/REPORT.md`, emitted milestone closeout artifacts, and regenerated the codebase-memory governance mirror. Graph import is not authorized by this milestone. Graph writes is disabled until a later production migration milestone provides per-paper atomic evidence.

## Cross-Slice Verification

- **S01 benchmark and reports:** `tests/test_m063_s01.py` verifies five candidate reports, 12-criteria scoring output, safety defaults, M045 trajectory evidence, and M050/M062 regression inputs.
- **S02 ADR-020:** `tests/test_m063_s02.py` verifies ADR-020 existence, M034-style section structure, explicit LadybugDB choice, migration plan, alternatives, ADR index update, and codebase-memory sync.
- **S03 closeout:** `tests/test_m063_s03.py` verifies REPORT.md, eight report sections, SUMMARY/VALIDATION files, LadybugDB choice documentation, ADR-020 references, codebase-memory sync, and M050/M063 S01/S02 regression.
- **Safety:** the benchmark and closeout preserve the five disabled defaults: network access, production import, graph writes, vendor-source mutation, and real DB connections.
- **Trajectory:** M045 remains `on_track`; M044 guardrail remains acceptable with graph import blocked and graph writes disabled.

## Requirement Changes

- GraphDB-selection: active → validated — `doc/adr/ADR-020-graphdb-selection.md` and `artifacts/m063-graphdb/scoring-matrix.md` select LadybugDB with a 39/45 score.
- Production graph migration: remains future work — M063 selects the target GraphDB but does not authorize production writes.
- Queue trigger evaluation: remains queued for M064 — ADR-018 and the M063 report keep queue work downstream of the selection result.

## Decision Re-evaluation

| Decision | Status | Re-evaluation |
|---|---|---|
| ADR-015 / ADR-016 NetworkX intermediate graph layer | retained | NetworkX remains the migration baseline and correctness reference. The requested ADR-015 file is not present under `doc/adr/`; ADR-020 references ADR-015 as indexed context while ADR-016 is the available project ADR. |
| ADR-018 M064 trigger evaluation | retained | M063 closes GraphDB selection, but does not invalidate the deferred queue decision. |
| ADR-020 GraphDB selection | accepted | LadybugDB is binding for the next production GraphDB migration work unless a later accepted ADR supersedes it. |

## Key Files

- `artifacts/m063-graphdb/REPORT.md` — Russian M063 synthesis report with sections 0-7.
- `artifacts/m063-graphdb/scoring-matrix.md` — 12-criteria comparison across five candidates.
- `artifacts/m063-graphdb/candidates/ladybugdb-report.md` — highest-scoring candidate report.
- `doc/adr/ADR-020-graphdb-selection.md` — binding GraphDB selection ADR.
- `.gsd/milestones/M065-u29n4f/M065-u29n4f-SUMMARY.md` — milestone closeout summary.
- `.gsd/milestones/M065-u29n4f/M065-u29n4f-VALIDATION.md` — closeout validation.
- `tests/test_m063_s03.py` — closeout and regression verification.

## Known Limitations

- M063 does not perform a live production LadybugDB import.
- M063 does not enable production graph writes.
- M063 does not decide queue implementation details; M064 remains the downstream queue milestone.
- The requested pre-read path `doc/adr/ADR-015-networkx-intermediate.md` is absent; available context comes from ADR-016 and ADR-020 references.

## Follow-ups

- M064 should use ADR-020 as an input when re-evaluating queue and orchestration boundaries.
- The first LadybugDB migration milestone should start with one paper, one atomic DAG, rollback evidence, and NetworkX parity checks.
- PostgreSQL-related choices should remain conditional until a later evidence line shows that graph metadata or storage boundaries require them.

## Lessons Learned

- The GraphDB decision is strongest when scored against daily-archive’s real graph shape rather than generic database popularity.
- Python-native migration friction matters as much as standalone graph-query maturity for the next project step.
- Binding ADRs need explicit downstream migration criteria so future agents know what was selected and what was not authorized.
- codebase-memory is useful as a recall mirror, but `.gsd/` and `doc/adr/` remain canonical.

## Completion State

M063 is complete at the selection-and-closeout level. It provides a binding GraphDB target, preserves safety defaults, and leaves production migration to a later milestone with explicit evidence gates.
