---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Draft locator protocol contract

Draft a protocol contract for candidate locators and chunk-span provenance. Define locator identity fields, source ledger fields, chunk/source coordinate fields, candidate evidence fields, uncertainty labels, review queue reasons, and explicit non-fact/import-disabled safety semantics.

## Inputs

- `M011 semantic gate findings`
- `M019 comparative matrix`
- `source and chunking artifact conventions from repository evidence`

## Expected Output

- `.gsd/milestones/M020-uh5kvt/slices/S01/candidate-locator-protocol.md`
- `.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-schema.json`

## Verification

uv run python inline assertions over candidate-locator-protocol-schema.json

## Observability Impact

Creates a reusable schema and contract surface for future locator runs.
