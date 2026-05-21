---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Write candidate locator tests

Write failing tests for deterministic candidate locator generation, source hash mismatch, broad-signal ambiguity, missing signal, forbidden payload detection, coordinate validation, and no-import safety flags.

## Inputs

- `.gsd/milestones/M021-xcfj4p/slices/S01/deterministic-locator-design.md`

## Expected Output

- `tests/test_candidate_locators.py`

## Verification

uv run pytest tests/test_candidate_locators.py -q should fail before implementation

## Observability Impact

Codifies expected diagnostics and safety invariants before implementation.
