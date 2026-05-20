---
id: S02
parent: M009-fh0tg0
milestone: M009-fh0tg0
provides:
  - CLI freshness verifier for S03 real-run provenance integration
  - sample pass/fail verifier artifacts
requires:
  - slice: S01
    provides: Provenance/freshness primitives and schemas.
affects:
  - S03
  - S04
  - S05
key_files:
  - src/arxiv_archive/cli.py
  - tests/test_validation_batch_cli_freshness.py
  - .gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-pass-report.json
  - .gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-stale-report.json
key_decisions:
  - Verifier exits 0 only for `fresh` verdict.
  - Verifier exits 1 for stale, missing, and invalid provenance.
  - Report writing is optional via `--report-path` and keeps output redacted.
patterns_established:
  - Artifact verification must include negative stale/missing tests.
  - Freshness reports are redacted diagnostic artifacts.
  - Exit code semantics distinguish trusted fresh artifacts from stale or invalid provenance.
observability_surfaces:
  - validation-batch verify-artifacts
  - freshness-pass-report.json
  - freshness-stale-report.json
drill_down_paths:
  - .gsd/milestones/M009-fh0tg0/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M009-fh0tg0/slices/S02/tasks/T02-SUMMARY.md
  - .gsd/milestones/M009-fh0tg0/slices/S02/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-20T04:47:14.320Z
blocker_discovered: false
---

# S02: Artifact freshness verifier

**S02 added the CLI artifact freshness verifier and proved fresh and stale outcomes.**

## What Happened

S02 added a `validation-batch verify-artifacts` command. It reads provenance JSONL, selects a run by run-id or batch/command, builds a freshness report, optionally writes it, and exits nonzero unless the verdict is fresh. CLI tests prove fresh pass, report writing, stale output mutation failure, missing output failure, input mutation failure, and redaction for invalid selection. Sample S02 evidence includes one fresh report and one intentionally stale report with `output_hash_changed` diagnostics.

## Verification

Fresh slice verification passed: pass report verdict fresh, stale report verdict stale with output hash/size diagnostics, safety flags false, 20 focused tests passed, and ruff passed.

## Requirements Advanced

- R036 — S02 exposes a CLI verifier that detects stale/missing/mutated artifacts from provenance logs.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

None. S02 verifies provenance logs created by S01 helpers; automatic provenance emission by init/preflight/scan remains later work.

## Known Limitations

The verifier relies on provenance logs; existing validation-batch init/preflight/scan still do not emit provenance automatically. S02 does not yet fix stale M006 metadata in scan summaries.

## Follow-ups

S03 should wire real validation-batch commands to emit provenance logs and add active milestone/batch metadata so the verifier can audit real runs, not just synthetic provenance fixtures.

## Files Created/Modified

- `src/arxiv_archive/cli.py` — Added verify-artifacts CLI command.
- `tests/test_validation_batch_cli_freshness.py` — Added CLI freshness verifier tests.
- `.gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-pass-report.json` — Fresh verifier sample report.
- `.gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-stale-report.json` — Intentional stale verifier sample report.
