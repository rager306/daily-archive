---
id: T02
parent: S03
milestone: M021-xcfj4p
key_files:
  - .gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch.json
  - .gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch-report.md
key_decisions: []
duration: 
verification_result: passed
completed_at: 2026-05-21T10:31:12.576Z
blocker_discovered: false
---

# T02: Ran deterministic bounded batch rehearsal over M011 targets.

**Ran deterministic bounded batch rehearsal over M011 targets.**

## What Happened

Ran the deterministic batch helper over the 10 M011 semantic review targets and persisted the S03 batch artifact and report. The run produced 26 locators, 19 ambiguous spans, 0 missing spans, 0 conflicting evidence, 7 retrieval-only locators, and 0 import-eligible or promoted facts. The output is route-filtered and reproducible.

## Verification

Verified with inline generation assertions and final S03 verification. Generation returned m021-s03-deterministic-batch-generated; final verification returned m021-s03-final-verification-ok.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python inline deterministic batch generation` | 0 | ✅ pass: m021-s03-deterministic-batch-generated | 5100ms |
| 2 | `uv run pytest tests/test_candidate_locators.py -q && uv run ruff check ... && uv run python inline S03 final verification` | 0 | ✅ pass: 10 passed; ruff clean; m021-s03-final-verification-ok | 5400ms |

## Deviations

None.

## Known Issues

The deterministic batch still has 19 ambiguous spans; semantic import remains blocked.

## Files Created/Modified

- `.gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch.json`
- `.gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch-report.md`
