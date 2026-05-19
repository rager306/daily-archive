---
id: T01
parent: S02
milestone: M007-opaont
key_files:
  - src/arxiv_archive/validation_batch_workflow.py
  - tests/test_validation_batch_workflow.py
key_decisions:
  - Keep S02 T01 workflow helpers pure: no acquisition, conversion, scan execution, KG import, or LadybugDB writes.
  - Normalize `m005_baseline_overlap` manifest roles to the M007 contract role `baseline_overlap`.
  - Treat Markdown quality acceptance in S02 preflight as existing non-empty Markdown only; richer quality checks belong in later source/preflight improvements.
duration: 
verification_result: passed
completed_at: 2026-05-19T19:04:20.221Z
blocker_discovered: false
---

# T01: Implemented pure batch initialization and source-preflight workflow helpers.

**Implemented pure batch initialization and source-preflight workflow helpers.**

## What Happened

Implemented pure validation batch workflow helpers. The module can load validation manifests, normalize selected paper roles, initialize a batch directory with batch-state and selection-manifest artifacts, inspect source paths for Markdown/PDF readiness, update validation batch state with source readiness, build source-preflight summaries, write diagnostics JSONL, and emit compact redacted state previews. Tests cover selection sorting/normalization, init artifact writing, source readiness detection, contradiction diagnostics, preflight summary writing, blocker/warning counts, and redacted preview output.

## Verification

Focused verification passed: 16 workflow/state tests passed and ruff reported all checks passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_validation_batch_workflow.py tests/test_validation_batch_state.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py src/arxiv_archive/validation_batch_state.py tests/test_validation_batch_workflow.py tests/test_validation_batch_state.py` | 0 | ✅ pass — 16 passed; ruff all checks passed | 5900ms |

## Deviations

Initial verification had a ruff import-order failure in the new workflow module; import ordering was fixed and verification reran successfully.

## Known Issues

Preflight quality acceptance is intentionally shallow in T01: it checks for present non-empty Markdown and path-level PDF state. It does not run conversion quality scoring or repair.

## Files Created/Modified

- `src/arxiv_archive/validation_batch_workflow.py`
- `tests/test_validation_batch_workflow.py`
