# S02: Deterministic locator module — UAT

**Milestone:** M021-xcfj4p
**Written:** 2026-05-21T10:19:41.480Z

# S02 UAT

## Scenario

A developer wants to generate a review-only candidate locator artifact from local source files.

## Expected behavior

- Import `arxiv_archive.candidate_locators`.
- Build an artifact using `build_candidate_locator_artifact` with `LocatorSource` and route specs.
- Artifact includes source ledger, locators, summary, and false safety flags.
- Artifact has no forbidden raw payload keys.
- Validation rejects import-eligible locators, fact promotion, true write flags, forbidden keys, and invalid coordinates.

## Evidence

```text
uv run pytest tests/test_candidate_locators.py -q
8 passed

uv run ruff check src/arxiv_archive/candidate_locators.py tests/test_candidate_locators.py
All checks passed!

m021-s02-module-guard-ok
```

## Verdict

PASS for module readiness. S03 can use the module for bounded batch rehearsal.

