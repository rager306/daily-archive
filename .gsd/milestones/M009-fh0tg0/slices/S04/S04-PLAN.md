# S04: Bounded quota top-up automation

**Goal:** Implement bounded quota top-up behavior so underfilled validation batches deterministically select replacement candidates or explicitly block scan.
**Demo:** After this slice, an underfilled validation batch deterministically draws replacements up to a bounded limit or blocks scan with a shortage report.

## Must-Haves

- Underfilled batch fixture produces rejected/deferred diagnostics and replacement candidates.
- Top-up selection is deterministic and respects max candidate/attempt bounds.
- Scan remains blocked until accepted_ready_count equals target_count.
- If quota cannot be filled, explicit blocker artifact is written.
- No acquisition is unbounded; no import/write behavior is added.

## Proof Level

- This slice proves: Shortage fixtures, bounded max-attempt tests, and sample pass/block artifacts.

## Integration Closure

Consumes quota-fill helper, candidate inventory, and source readiness. Produces accepted/rejected/replacement diagnostics for S05 review and future batch execution.

## Verification

- Adds top-up summary and diagnostics with target, accepted, rejected, replacement, shortage, max-attempt, and scan_allowed fields.

## Tasks

- [x] **T01: Implement bounded top up planner** `est:medium`
  Add bounded top-up planning helpers that consume current batch state plus candidate inventory/readiness metadata and produce a redacted top-up report. The helper should not acquire sources; it plans deterministic replacements within max_candidates_to_consider and computes scan_allowed.
  - Files: `src/arxiv_archive/validation_batch_workflow.py`
  - Verify: uv run pytest tests/test_validation_batch_quota_fill.py tests/test_validation_batch_top_up.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py tests/test_validation_batch_top_up.py

- [x] **T02: Test bounded top up behavior** `est:medium`
  Add tests for top-up planning: already full quota, underfilled with enough replacements, underfilled with max-attempt shortage, duplicate/selected candidate exclusion, and redaction/safety flags.
  - Files: `tests/test_validation_batch_top_up.py`
  - Verify: uv run pytest tests/test_validation_batch_top_up.py tests/test_validation_batch_quota_fill.py -q && uv run ruff check tests/test_validation_batch_top_up.py src/arxiv_archive/validation_batch_workflow.py

- [x] **T03: Run top up regression and sample evidence** `est:small`
  Generate S04 sample evidence for a successful top-up plan and a blocked shortage plan, then run focused regression.
  - Files: `.gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-pass-summary.json`, `.gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-blocked-summary.json`
  - Verify: uv run pytest tests/test_validation_batch_top_up.py tests/test_validation_batch_quota_fill.py tests/test_validation_batch_scan_workflow.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py tests/test_validation_batch_top_up.py && test -s .gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-pass-summary.json && test -s .gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-blocked-summary.json

## Files Likely Touched

- src/arxiv_archive/validation_batch_workflow.py
- tests/test_validation_batch_top_up.py
- .gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-pass-summary.json
- .gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-blocked-summary.json
