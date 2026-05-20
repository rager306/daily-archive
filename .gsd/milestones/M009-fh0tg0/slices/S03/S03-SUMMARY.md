---
id: S03
parent: M009-fh0tg0
milestone: M009-fh0tg0
provides:
  - active scan lineage support
  - metadata mismatch freshness checks
  - lineage pass/fail sample evidence
requires:
  - slice: S01
    provides: Provenance/freshness primitives.
  - slice: S02
    provides: CLI artifact verifier.
affects:
  - S04
  - S05
key_files:
  - src/arxiv_archive/validation_batch_workflow.py
  - src/arxiv_archive/validation_batch_provenance.py
  - src/arxiv_archive/cli.py
  - .gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-pass-report.json
  - .gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-mismatch-report.json
key_decisions:
  - Make `--milestone-id` opt-in for validation-batch scan to preserve existing behavior.
  - When provided, active milestone_id overrides stale scanner `milestone` metadata in validation-batch summary artifacts.
  - Metadata mismatch is a stale freshness verdict even if file hashes match the recorded output.
patterns_established:
  - Lineage metadata is part of artifact freshness, not just human review.
  - Validation-batch scan should be invoked with active `--milestone-id` for future auditable runs.
  - Hash-valid artifacts can still be stale if their embedded milestone/batch metadata is wrong.
observability_surfaces:
  - active milestone_id/batch_id in scan artifacts
  - artifact_metadata_mismatch diagnostics
  - lineage-pass-report.json
  - lineage-mismatch-report.json
drill_down_paths:
  - .gsd/milestones/M009-fh0tg0/slices/S03/tasks/T01-SUMMARY.md
  - .gsd/milestones/M009-fh0tg0/slices/S03/tasks/T02-SUMMARY.md
  - .gsd/milestones/M009-fh0tg0/slices/S03/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-20T05:15:47.371Z
blocker_discovered: false
---

# S03: Active scan lineage metadata

**S03 added active scan lineage metadata and verifier checks that catch stale M006-style artifact metadata.**

## What Happened

S03 fixed the stale lineage problem identified in M008 review. Validation-batch scan can now accept an active milestone id and stamp `milestone_id` plus `batch_id` into scan manifest, source-readiness, summary, delta, and outlier artifacts. When active lineage is supplied, the summary `milestone` field is also set to the active milestone instead of the reused scanner's stale M006 value. The provenance freshness verifier now supports expected artifact metadata and emits `artifact_metadata_mismatch` as a stale verdict when JSON artifacts carry the wrong milestone/batch lineage. Sample evidence demonstrates both a fresh M009 lineage report and a stale M006-style mismatch report.

## Verification

Fresh slice verification passed: lineage pass report fresh, lineage mismatch report stale with artifact_metadata_mismatch, safety flags false, 19 focused tests passed, and ruff passed.

## Requirements Advanced

- R036 — S03 adds active artifact lineage metadata and verifier mismatch detection for validation CLI outputs.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

None.

## Known Limitations

Real validation-batch commands still do not emit provenance logs automatically. S03 fixes lineage metadata support and verifier checks, but S04 still needs bounded quota top-up behavior.

## Follow-ups

S04 can now rely on active lineage metadata and freshness verifier primitives while implementing bounded quota top-up automation.

## Files Created/Modified

- `src/arxiv_archive/validation_batch_workflow.py` — Active scan lineage metadata support.
- `src/arxiv_archive/cli.py` — CLI `--milestone-id` scan option.
- `src/arxiv_archive/validation_batch_provenance.py` — Provenance metadata mismatch verification.
- `tests/test_validation_batch_provenance.py` — Lineage metadata tests.
- `tests/test_validation_batch_scan_workflow.py` — Scan lineage tests.
- `tests/test_validation_batch_cli_scan.py` — CLI scan lineage tests.
- `.gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-pass-report.json` — Lineage pass sample report.
- `.gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-mismatch-report.json` — Lineage mismatch sample report.
