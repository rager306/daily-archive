---
id: T01
parent: S02
milestone: M020-uh5kvt
key_files:
  - .gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-fixture.json
  - .gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-report.md
key_decisions: []
duration: 
verification_result: passed
completed_at: 2026-05-21T09:21:16.156Z
blocker_discovered: false
---

# T01: Generated a one-paper candidate locator fixture for 2001.00281v1.

**Generated a one-paper candidate locator fixture for 2001.00281v1.**

## What Happened

Selected M011 target 2001.00281v1 because its source artifact exists, hash matches, and M011 metadata marks it as claim/method-heavy with import_ready=false. Generated a one-paper candidate locator fixture under the S01 protocol using only source paths, hashes, redacted coordinates, line offsets, span hashes, categorical diagnostics, and aggregate M011 metadata. All locators remain import-disabled and not promoted to facts.

## Verification

Verified with uv run python inline fixture generation and final S02 assertions. Fresh final verification returned m020-s02-final-verification-ok.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python inline fixture generation` | 0 | ✅ pass: m020-s02-one-paper-fixture-generated | 3900ms |
| 2 | `uv run python inline S02 final verification` | 0 | ✅ pass: m020-s02-final-verification-ok | 13800ms |

## Deviations

None.

## Known Issues

The fixture does not prove semantic fact correctness; semantic review remains required by design.

## Files Created/Modified

- `.gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-fixture.json`
- `.gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-report.md`
