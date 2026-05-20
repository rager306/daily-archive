---
id: S03
parent: M007-opaont
milestone: M007-opaont
provides:
  - validation-batch scan command
  - scan/delta/outlier workflow helpers
  - 30-paper automated scan evidence
  - import-gate blocker surface
requires:
  - slice: S02
    provides: Source-ready batch state and source preflight artifacts.
affects:
  - S04
  - future +10 batch execution
key_files:
  - src/arxiv_archive/validation_batch_workflow.py
  - src/arxiv_archive/cli.py
  - .gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-summary.json
  - .gsd/milestones/M007-opaont/slices/S03/run-evidence/delta-report.json
  - .gsd/milestones/M007-opaont/slices/S03/validation-scan-report.md
key_decisions:
  - Use existing redacted deviation scanner rather than duplicate scan logic.
  - Separate M005/S03 apples-to-apples baseline from M005/S06 mixed benchmark context.
  - Keep non-zero import eligibility as a blocker diagnostic and review gate.
  - Treat S03 as automation proof over the existing 30-paper batch, not as a new +10 corpus expansion.
patterns_established:
  - Automated scan artifacts use M007 names while reusing existing redacted scanner logic.
  - Baseline comparisons must preserve M005/S03 versus M005/S06 semantics.
  - Any future non-zero import eligibility is an automation blocker, not a success signal.
observability_surfaces:
  - validation-scan-summary.json captures aggregate scan state and safety flags.
  - validation-scan-diagnostics.jsonl captures per-paper redacted scan metrics.
  - delta-report.json captures route/refusal deltas and separate baselines.
  - outlier-report.json captures thresholds and outliers with normalized density.
  - batch-state.json records scan artifact paths and phase.
drill_down_paths:
  - .gsd/milestones/M007-opaont/slices/S03/tasks/T01-SUMMARY.md
  - .gsd/milestones/M007-opaont/slices/S03/tasks/T02-SUMMARY.md
  - .gsd/milestones/M007-opaont/slices/S03/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-20T01:57:42.918Z
blocker_discovered: false
---

# S03: Automated scan delta and outlier gates

**S03 automated the 30-paper scan/delta/outlier evidence path through validation-batch scan.**

## What Happened

S03 automated the validation scan evidence path. It added scan workflow helpers, wired `validation-batch scan`, and ran a bounded dry run over the S02 30-paper state. The generated artifacts match M006 evidence: 30 papers, 4,289 chunks, zero import-eligible chunks, and 11 outliers. The delta report correctly separates M005/S03 structure-aware baseline (+2,458 chunks) from M005/S06 mixed benchmark context (+1,818 chunks). The workflow updates batch artifact paths and would add a blocker diagnostic if future scans produce non-zero import eligibility outside a reviewed promotion path. No raw/chunk text, embeddings/vectors, KG import, or LadybugDB writes were emitted.

## Verification

Fresh slice verification passed: 56 tests passed, ruff passed, and artifact guard confirmed 30 papers, 4,289 chunks, 11 outliers, zero import eligibility, correct baseline deltas, and all safety flags false.

## Requirements Advanced

- R033 — S03 implements automated batch scan execution, delta reporting, outlier reporting, and import-gate status.
- R032 — S03 advances the +10-to-100 loop by automating the scan-analysis stage after source preflight.
- R029 — S03 keeps route/refusal evidence separate from trusted KG import and semantic correctness claims.

## Requirements Validated

None.

## New Requirements Surfaced

- Future CLI ergonomics should return structured JSON errors for invalid phases when `--json` is requested.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

The first S03 dry-run delta report misread the M005/S06 mixed benchmark because the benchmark count is nested under `aggregate.total_chunk_count`. The parser and tests were fixed, and the evidence was regenerated with the correct +1,818 mixed benchmark delta.

## Known Limitations

S03 does not select new papers or run a new +10 batch. It proves the automated scan/delta/outlier path against the existing 30-paper batch. CLI unready-state errors are not yet structured JSON errors.

## Follow-ups

S04 should independently review whether S01-S03 artifacts are sufficient to run a new +10 batch, and whether JSON error payloads are needed for unready scan states.

## Files Created/Modified

- `src/arxiv_archive/validation_batch_workflow.py` — Validation-batch scan workflow helpers, delta/outlier reports, and import-gate diagnostics.
- `src/arxiv_archive/cli.py` — CLI wiring for validation-batch scan.
- `tests/test_validation_batch_scan_workflow.py` — Scan workflow tests.
- `tests/test_validation_batch_cli_scan.py` — CLI scan tests.
- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-summary.json` — Validation scan summary.
- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/delta-report.json` — Delta report.
- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/outlier-report.json` — Outlier report.
- `.gsd/milestones/M007-opaont/slices/S03/validation-scan-report.md` — Validation scan dry-run report.
