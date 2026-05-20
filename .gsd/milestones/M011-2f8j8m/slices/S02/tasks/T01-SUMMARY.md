---
id: T01
parent: S02
milestone: M011-2f8j8m
key_files:
  - .gsd/milestones/M011-2f8j8m/slices/S02/semantic-review-rubric.md
key_decisions:
  - Make missing chunk-level source spans an import-blocking condition for M010-derived targets.
  - Allow retrieval_only and repair_required recommendations, but require future scoped rehearsal before any positive import.
duration: 
verification_result: passed
completed_at: 2026-05-20T08:26:05.740Z
blocker_discovered: false
---

# T01: Defined a conservative semantic import-readiness rubric that blocks import candidates without chunk-level span provenance.

**Defined a conservative semantic import-readiness rubric that blocks import candidates without chunk-level span provenance.**

## What Happened

Wrote the M011 semantic import-readiness rubric. The rubric defines allowed and prohibited evidence, classification labels, review dimensions, and a conservative M010-specific rule: because M010 diagnostics are paper-level aggregate records, targets with missing chunk-level spans cannot be classified as import_candidate. It preserves the safety invariant that S02 may recommend future work but must not create trusted KG facts, perform positive import, or write to production LadybugDB.

## Verification

semantic-review-rubric.md exists.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `test -s .gsd/milestones/M011-2f8j8m/slices/S02/semantic-review-rubric.md` | 0 | ✅ pass — rubric exists | 5600ms |

## Deviations

None.

## Known Issues

The rubric is conservative and is expected to block import_candidate classifications for M010 unless additional chunk-span evidence is produced.

## Files Created/Modified

- `.gsd/milestones/M011-2f8j8m/slices/S02/semantic-review-rubric.md`
