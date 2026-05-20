---
id: T03
parent: S01
milestone: M009-fh0tg0
key_files:
  - .gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-cli-run-log.jsonl
  - .gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-freshness-report.json
key_decisions:
  - Sample run evidence is synthetic and explicitly scoped to S01 provenance shape, not a real validation-batch scan.
  - Freshness report verdict `fresh` demonstrates that unchanged hashed files match the provenance entry.
duration: 
verification_result: passed
completed_at: 2026-05-20T04:39:53.662Z
blocker_discovered: false
---

# T03: Generated sample provenance/freshness artifacts and verified regression tests.

**Generated sample provenance/freshness artifacts and verified regression tests.**

## What Happened

Generated commit-safe sample provenance artifacts under S01 run-evidence and ran regression checks. The sample CLI run log records a synthetic `validation-batch scan` command over sample JSON inputs/outputs and the freshness report verifies the recorded files as fresh. Regression tests confirmed the new module does not affect existing validation batch workflow behavior.

## Verification

Sample run log and freshness report exist. Provenance and workflow tests passed, and ruff passed for touched files.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `generate sample-cli-run-log.jsonl/sample-freshness-report.json; uv run pytest tests/test_validation_batch_provenance.py tests/test_validation_batch_workflow.py -q; uv run ruff check ...` | 0 | ✅ pass — freshness verdict fresh; 18 tests passed; ruff passed | 10100ms |

## Deviations

None.

## Known Issues

S01 sample artifacts prove the provenance shape only. S02 must add a CLI verifier and S03/S04 must integrate this with real validation-batch commands/top-up behavior.

## Files Created/Modified

- `.gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-cli-run-log.jsonl`
- `.gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-freshness-report.json`
