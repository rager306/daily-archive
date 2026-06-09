# S01: S01

**Goal:** Map the M020 protocol into a minimal implementation design and test contract before editing code.
**Demo:** After S01, there is a designed code API for deterministic candidate locators with clear safety invariants and tests planned.

## Must-Haves

- Existing evidence/chunk/import-boundary primitives inspected.
- Implementation API and diagnostic model documented.
- GitNexus impact analysis recorded for symbols to edit/create.
- No code behavior changed yet.

## Proof Level

- This slice proves: Design artifact plus impact analysis.

## Integration Closure

Design feeds S02 implementation.

## Verification

- Defines diagnostics and safety surfaces for implementation.

## Tasks

- [x] **T01: Designed the deterministic candidate locator implementation boundary.** `est:60m`
  Inspect existing evidence, import-boundary, validation, and CLI patterns. Produce a design note for the candidate locator module API, data structures, diagnostics, safety flags, and tests. Include explicit non-goals and no-import semantics.
  - Files: `.gsd/milestones/M021-xcfj4p/slices/S01/deterministic-locator-design.md`
  - Verify: design artifact contains API, diagnostics, safety flags, test plan, and no-import semantics

- [x] **T02: Recorded implementation impact map and additive edit boundary.** `est:30m`
  Run GitNexus context and impact analysis for relevant existing symbols before implementation. Record affected callers/processes and proposed edit targets. The expected plan is to add a new module and tests, with minimal or no edits to existing symbols.
  - Files: `.gsd/milestones/M021-xcfj4p/slices/S01/implementation-impact-map.md`
  - Verify: impact map references GitNexus impact results and proposed edit boundaries

## Files Likely Touched

- .gsd/milestones/M021-xcfj4p/slices/S01/deterministic-locator-design.md
- .gsd/milestones/M021-xcfj4p/slices/S01/implementation-impact-map.md
