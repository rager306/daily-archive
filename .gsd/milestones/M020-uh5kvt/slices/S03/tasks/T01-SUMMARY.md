---
id: T01
parent: S03
milestone: M020-uh5kvt
key_files:
  - .gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal.json
  - .gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal-report.md
key_decisions: []
duration: 
verification_result: passed
completed_at: 2026-05-21T09:26:59.374Z
blocker_discovered: false
---

# T01: Generated the small-batch locator rehearsal over 10 M011 targets.

**Generated the small-batch locator rehearsal over 10 M011 targets.**

## What Happened

Generated a bounded small-batch candidate locator rehearsal across all 10 M011 targets. The rehearsal uses the S01 protocol and S02 fixture shape, producing source ledger entries, redacted coordinate-bearing locators, per-paper summaries, aggregate counts, and safety flags. It records paths, hashes, offsets, line numbers, span hashes, and categorical diagnostics only.

## Verification

Verified with uv run python inline rehearsal generation and final S03 assertions. Fresh verification returned m020-s03-final-verification-ok.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python inline small-batch rehearsal generation` | 0 | ✅ pass: m020-s03-small-batch-rehearsal-generated | 8500ms |
| 2 | `uv run python inline S03 final verification` | 0 | ✅ pass: m020-s03-final-verification-ok | 7700ms |

## Deviations

None.

## Known Issues

The batch uses heuristic coordinate locators and yields many ambiguous spans; this is expected and requires S04 independent semantic review.

## Files Created/Modified

- `.gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal.json`
- `.gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal-report.md`
