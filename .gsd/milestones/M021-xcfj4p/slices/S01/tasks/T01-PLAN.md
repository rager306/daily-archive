---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Designed the deterministic candidate locator implementation boundary.

Inspect existing evidence, import-boundary, validation, and CLI patterns. Produce a design note for the candidate locator module API, data structures, diagnostics, safety flags, and tests. Include explicit non-goals and no-import semantics.

## Inputs

- `src/arxiv_archive/evidence.py`
- `src/arxiv_archive/import_boundary_rehearsal.py`
- `src/arxiv_archive/validation_batch_workflow.py`
- `.gsd/milestones/M020-uh5kvt/slices/S01/candidate-locator-protocol.md`
- `.gsd/milestones/M020-uh5kvt/slices/S03/small-batch-rehearsal-recommendation.md`

## Expected Output

- `.gsd/milestones/M021-xcfj4p/slices/S01/deterministic-locator-design.md`

## Verification

design artifact contains API, diagnostics, safety flags, test plan, and no-import semantics

## Observability Impact

Design defines diagnostic codes and guard surfaces for implementation.
