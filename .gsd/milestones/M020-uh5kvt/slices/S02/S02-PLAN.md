# S02: One-paper locator fixture

**Goal:** Exercise the locator protocol on one paper fixture to prove the contract can represent useful candidate evidence without promoting facts.
**Demo:** After S02, one known paper has candidate locators with exact chunk-span/source-span provenance under the S01 contract and import disabled.

## Must-Haves

- One paper fixture selected from existing source artifacts.
- Candidate locator JSON conforms to S01 contract.
- Exact source-span coordinates are present.
- Import eligibility remains false.
- Machine artifacts avoid raw paper/chunk text.

## Proof Level

- This slice proves: Fixture artifact plus targeted semantic spot check.

## Integration Closure

One-paper output feeds small-batch rehearsal design in S03.

## Verification

- Produces locator artifact, diagnostics, and guard for a single paper.

## Tasks

- [x] **T01: Generate one-paper locator fixture** `est:60m`
  Select one M011/M010 source-backed target and generate a candidate locator fixture under the S01 protocol. The fixture should use source path/hash and exact redacted coordinates only, with locator states that remain review-only and import-disabled.
  - Files: `.gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-fixture.json`, `.gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-report.md`
  - Verify: uv run python inline fixture schema and safety assertions

- [x] **T02: Validate one-paper locator fixture** `est:45m`
  Validate the one-paper fixture against the protocol guard and perform a targeted semantic spot check that records only categorical judgments and coordinates, not raw text.
  - Files: `.gsd/milestones/M020-uh5kvt/slices/S02/run-evidence/one-paper-locator-guard.json`, `.gsd/milestones/M020-uh5kvt/slices/S02/one-paper-semantic-spot-check.md`
  - Verify: uv run python inline guard assertions and no-raw-payload scan

## Files Likely Touched

- .gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-fixture.json
- .gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-report.md
- .gsd/milestones/M020-uh5kvt/slices/S02/run-evidence/one-paper-locator-guard.json
- .gsd/milestones/M020-uh5kvt/slices/S02/one-paper-semantic-spot-check.md
