# S01: S01

**Goal:** Define the candidate locator and chunk-span provenance contract before generating any locators.
**Demo:** After S01, daily-archive has a protocol contract for candidate locators, source spans, uncertainty, review queues, and import-disabled safety flags.

## Must-Haves

- Candidate locator schema defined.
- Source span coordinate fields defined.
- Uncertainty labels and review queue reasons defined.
- Safety flags block production import and LadybugDB writes.
- No source corpus or generated KG facts are produced.

## Proof Level

- This slice proves: Contract artifact plus JSON guard assertions.

## Integration Closure

The contract is the input for one-paper fixture generation in S02.

## Verification

- Adds schema and guard artifacts future agents can validate before running locator workflows.

## Tasks

- [x] **T01: Defined the M020 candidate locator and chunk-span provenance protocol contract.** `est:60m`
  Draft a protocol contract for candidate locators and chunk-span provenance. Define locator identity fields, source ledger fields, chunk/source coordinate fields, candidate evidence fields, uncertainty labels, review queue reasons, and explicit non-fact/import-disabled safety semantics.
  - Files: `.gsd/milestones/M020-uh5kvt/slices/S01/candidate-locator-protocol.md`, `.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-schema.json`
  - Verify: uv run python inline assertions over candidate-locator-protocol-schema.json

- [x] **T02: Validated the locator protocol safety guard.** `est:45m`
  Write a protocol guard and validation report proving the schema blocks fact promotion, production import, LadybugDB writes, raw corpus persistence, and MiniMax authority behavior. Validate S01 against guard assertions.
  - Files: `.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-guard.json`, `.gsd/milestones/M020-uh5kvt/slices/S01/protocol-validation-report.md`
  - Verify: uv run python inline assertions over protocol guard and schema

## Files Likely Touched

- .gsd/milestones/M020-uh5kvt/slices/S01/candidate-locator-protocol.md
- .gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-schema.json
- .gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-guard.json
- .gsd/milestones/M020-uh5kvt/slices/S01/protocol-validation-report.md
