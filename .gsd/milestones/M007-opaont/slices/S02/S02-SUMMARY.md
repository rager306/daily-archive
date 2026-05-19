---
id: S02
parent: M007-opaont
milestone: M007-opaont
provides:
  - Initialized validation batch artifacts
  - 30-paper source preflight summary
  - Readiness/risk-tag contradiction diagnostics
  - CLI foundation for S03 scan automation
requires:
  - slice: S01
    provides: State schema, safety flags, CLI contract, and command namespace.
affects:
  - S03
  - S04
key_files:
  - src/arxiv_archive/validation_batch_workflow.py
  - src/arxiv_archive/cli.py
  - .gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-summary.json
  - .gsd/milestones/M007-opaont/slices/S02/source-preflight-report.md
key_decisions:
  - Implement `init` and `preflight` as real local artifact-writing commands.
  - Preserve scan/review/resume as non-zero stubs until later slices.
  - Use deterministic fallback source/cache paths so preflight reflects current source state, not stale manifest paths only.
  - Surface historical `missing_markdown` tags as warnings, not blockers, when Markdown is now present and accepted.
patterns_established:
  - Current source state should use deterministic fallback paths in addition to manifest paths.
  - Historical risk tags should surface as warnings until explicitly resolved.
  - Batch commands should emit compact JSON previews plus durable artifacts.
observability_surfaces:
  - batch-state.json captures phase and selected paper state.
  - source-preflight-summary.json exposes aggregate readiness counts and safety flags.
  - source-preflight-diagnostics.jsonl exposes per-paper contradiction warnings.
  - source-preflight-report.md explains warnings and next action.
drill_down_paths:
  - .gsd/milestones/M007-opaont/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M007-opaont/slices/S02/tasks/T02-SUMMARY.md
  - .gsd/milestones/M007-opaont/slices/S02/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-19T19:15:23.164Z
blocker_discovered: false
---

# S02: Batch initialization and source preflight

**S02 made validation-batch init/preflight real and produced 30-paper source readiness artifacts.**

## What Happened

S02 implemented real deterministic validation-batch initialization and source preflight. The CLI can now initialize a batch from a manifest and write batch-state plus selection manifest artifacts. It can then preflight source paths, update state, write source readiness summary, and emit contradiction diagnostics. The 30-paper dry run reports 30/30 Markdown-ready, 8 PDFs present, 22 PDFs missing, 20 historical missing-Markdown warnings, and 0 blockers. The safety boundary remains intact: no acquisition, conversion, scan, KG import, or LadybugDB write occurred.

## Verification

Fresh slice verification passed: 51 focused/regression tests passed, ruff passed, and artifact guard confirmed 30 papers, 30 Markdown-ready, 8 PDFs present, 20 warnings, 0 blockers, and all safety flags false.

## Requirements Advanced

- R033 — S02 implements deterministic batch initialization and source preflight, a core part of the resumable CLI workflow.
- R032 — S02 advances the +10-to-100 automation loop by producing source preflight artifacts that can gate scan execution.
- R030 — S02 preserves source/PDF caveats separately from Markdown-scan readiness.

## Requirements Validated

None.

## New Requirements Surfaced

- Future quality gates should add richer Markdown quality scoring and explicit historical-risk-tag resolution fields.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

The first T03 dry run returned only 10/30 Markdown-ready because source preflight trusted stale manifest paths. The workflow helper was corrected to use deterministic fallback source/cache paths, after which the run produced 30/30 Markdown-ready.

## Known Limitations

S02 does not acquire missing sources, convert PDFs, score Markdown quality beyond non-empty presence, run deviation scans, or perform review decisions. PDF completeness remains 8/30.

## Follow-ups

S03 should consume the S02 batch-state/source-preflight artifacts, run the existing deviation scanner through the validation-batch workflow, and emit delta/outlier gates. It should preserve the 20 readiness/risk-tag warnings as review context.

## Files Created/Modified

- `src/arxiv_archive/validation_batch_workflow.py` — Batch workflow helpers for manifest loading, init, preflight, summaries, diagnostics, and previews.
- `src/arxiv_archive/cli.py` — CLI wiring for validation-batch init and preflight.
- `tests/test_validation_batch_workflow.py` — Workflow helper tests.
- `tests/test_validation_batch_cli_preflight.py` — CLI preflight tests.
- `.gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-summary.json` — Dry-run source preflight summary.
- `.gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-diagnostics.jsonl` — Dry-run source preflight diagnostics.
- `.gsd/milestones/M007-opaont/slices/S02/source-preflight-report.md` — Dry-run report.
