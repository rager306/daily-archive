---
id: S01
parent: M009-fh0tg0
milestone: M009-fh0tg0
provides:
  - provenance module for S02 verifier
  - sample run log schema
  - freshness report schema
requires:
  []
affects:
  - S02
  - S03
  - S04
key_files:
  - src/arxiv_archive/validation_batch_provenance.py
  - tests/test_validation_batch_provenance.py
  - .gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-cli-run-log.jsonl
  - .gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-freshness-report.json
key_decisions:
  - Keep provenance external to ValidationBatchState to avoid schema churn.
  - Use sha256 and size as freshness authority; mtime is contextual.
  - Redact secret-like argv values defensively even though validation-batch commands currently have no secret options.
  - Do not fake stdout/stderr capture; paths are nullable until explicit capture exists.
patterns_established:
  - Provenance is an audit layer outside validation batch state.
  - Artifact freshness is hash/size based and redacted.
  - Negative freshness cases are first-class tests, not manual review only.
observability_surfaces:
  - sample-cli-run-log.jsonl
  - sample-freshness-report.json
drill_down_paths:
  - .gsd/milestones/M009-fh0tg0/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M009-fh0tg0/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M009-fh0tg0/slices/S01/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-20T04:41:55.621Z
blocker_discovered: false
---

# S01: CLI run provenance log

**S01 added commit-safe provenance/freshness primitives with tests and sample artifacts.**

## What Happened

S01 added a new isolated validation-batch provenance module and tests. The module can hash files without serializing contents, redact secret-like CLI arguments, capture command/run context and safety flags, append/read JSONL provenance, select a provenance entry, and build/write freshness reports that detect stale, missing, or unsafe artifacts. Unit tests cover positive and negative freshness cases and raw-content sentinel non-leakage. A sample run log and freshness report were generated as commit-safe evidence.

## Verification

Fresh slice verification passed: provenance schema present, freshness verdict fresh, input/output hashes recorded, safety flags false, 18 focused tests passed, and ruff passed.

## Requirements Advanced

- R036 — S01 adds provenance/freshness primitives and sample artifacts.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

None. CLI integration was intentionally deferred; S01 is library primitives plus sample artifacts only.

## Known Limitations

No CLI command writes provenance yet; sample artifacts are synthetic. Freshness proves file identity, not semantic correctness. Lineage metadata checking against active milestone is planned for S02/S03.

## Follow-ups

S02 should add CLI-facing artifact verification over these primitives. Later slices should wire real validation-batch commands to write provenance logs and fix scan lineage metadata.

## Files Created/Modified

- `src/arxiv_archive/validation_batch_provenance.py` — New provenance and freshness helper module.
- `tests/test_validation_batch_provenance.py` — Unit tests for provenance and freshness behavior.
- `.gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-cli-run-log.jsonl` — Sample provenance log.
- `.gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-freshness-report.json` — Sample freshness report.
