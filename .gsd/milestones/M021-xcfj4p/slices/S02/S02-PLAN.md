# S02: Deterministic locator module

**Goal:** Implement deterministic candidate locator module and tests for protocol-conformant artifacts.
**Demo:** After S02, unit-tested code can build candidate locator artifacts for source-backed targets with no import/write/raw payload behavior.

## Must-Haves

- Locator module implemented.
- Tests cover source hash checks, coordinate validation, ambiguity classes, and safety flags.
- Forbidden payload keys/raw text serialization rejected.
- All locators remain import-disabled and not promoted to facts.

## Proof Level

- This slice proves: Unit tests plus ruff/lsp diagnostics.

## Integration Closure

Module provides callable API for S03 batch CLI/rehearsal.

## Verification

- Adds structured ambiguity diagnostics and safety guard serialization.

## Tasks

- [x] **T01: Write candidate locator tests** `est:60m`
  Write failing tests for deterministic candidate locator generation, source hash mismatch, broad-signal ambiguity, missing signal, forbidden payload detection, coordinate validation, and no-import safety flags.
  - Files: `tests/test_candidate_locators.py`
  - Verify: uv run pytest tests/test_candidate_locators.py -q should fail before implementation

- [x] **T02: Implement candidate locator module** `est:90m`
  Implement `src/arxiv_archive/candidate_locators.py` to satisfy the tests with deterministic protocol-conformant artifacts and recursive safety validation.
  - Files: `src/arxiv_archive/candidate_locators.py`
  - Verify: uv run pytest tests/test_candidate_locators.py -q && uv run ruff check src/arxiv_archive/candidate_locators.py tests/test_candidate_locators.py

- [x] **T03: Verify candidate locator module** `est:30m`
  Run focused verification and diagnostics for changed files, including pytest, ruff, and LSP diagnostics if available.
  - Files: `.gsd/milestones/M021-xcfj4p/slices/S02/run-evidence/candidate-locator-module-guard.json`
  - Verify: uv run pytest tests/test_candidate_locators.py -q && uv run ruff check src/arxiv_archive/candidate_locators.py tests/test_candidate_locators.py && uv run python inline guard

## Files Likely Touched

- tests/test_candidate_locators.py
- src/arxiv_archive/candidate_locators.py
- .gsd/milestones/M021-xcfj4p/slices/S02/run-evidence/candidate-locator-module-guard.json
