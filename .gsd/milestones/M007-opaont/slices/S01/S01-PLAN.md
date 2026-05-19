# S01: CLI contract and batch state model

**Goal:** Design and implement the first safe contract layer for iterative validation batches: state schema, contradiction diagnostics, documented CLI command surface, and fixture tests before wiring real acquisition/scanning commands.
**Demo:** After this slice, the project has a documented CLI contract, command names, state schema, fixture shape, and safety gates for iterative validation batches.

## Must-Haves

- CLI command surface and batch state schema are documented.
- State helpers represent selection, source readiness, scan, review, and recommendation phases.
- Safety flags encode no raw/chunk text, no embeddings/vectors, no imports/writes.
- Contradiction diagnostics catch readiness/risk-tag mismatches.
- Focused tests and ruff pass.

## Proof Level

- This slice proves: Design document plus focused tests for state helpers and contradiction detection.

## Integration Closure

Consumes M006 S04 recommendation and produces contract helpers that S02/S03 will use for source preflight and scan automation.

## Verification

- Adds explicit batch state validation, safety flags, contradiction diagnostics, and command contract documentation.

## Tasks

- [x] **T01: Document validation CLI contract** `est:small`
  Draft the M007 CLI contract and batch state schema. Include command names, artifact paths, phase model, safety boundaries, and out-of-scope KG import/promotion.
  - Files: `.gsd/milestones/M007-opaont/slices/S01/validation-cli-contract.md`
  - Verify: test -s .gsd/milestones/M007-opaont/slices/S01/validation-cli-contract.md && grep -q 'No production KG import' .gsd/milestones/M007-opaont/slices/S01/validation-cli-contract.md

- [x] **T02: Implement batch state helpers** `est:medium`
  Implement a small batch state module with dataclasses or typed helpers for validation batch state, safety flags, state serialization, and contradiction diagnostics. Avoid wiring real CLI commands yet.
  - Files: `src/arxiv_archive/validation_batch_state.py`, `tests/test_validation_batch_state.py`
  - Verify: uv run pytest tests/test_validation_batch_state.py -q && uv run ruff check src/arxiv_archive/validation_batch_state.py tests/test_validation_batch_state.py

- [x] **T03: Expose validation batch CLI contract stub** `est:medium`
  Add a narrow CLI contract stub or help surface for future validation batch commands without running real batch work. The command should expose intended subcommands and return a safe not-implemented/contract response where appropriate.
  - Files: `src/arxiv_archive/cli.py`, `tests/test_validation_batch_cli_contract.py`
  - Verify: uv run pytest tests/test_validation_batch_state.py tests/test_validation_batch_cli_contract.py -q && uv run ruff check src/arxiv_archive/cli.py src/arxiv_archive/validation_batch_state.py tests/test_validation_batch_state.py tests/test_validation_batch_cli_contract.py

## Files Likely Touched

- .gsd/milestones/M007-opaont/slices/S01/validation-cli-contract.md
- src/arxiv_archive/validation_batch_state.py
- tests/test_validation_batch_state.py
- src/arxiv_archive/cli.py
- tests/test_validation_batch_cli_contract.py
