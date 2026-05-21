---
id: T01
parent: S02
milestone: M017-cf3fd0
key_files:
  - src/arxiv_archive/minimax_usage.py
  - tests/test_minimax_usage.py
  - .gsd/milestones/M017-cf3fd0/slices/S02/run-evidence/minimax-usage-helper-guard.json
key_decisions:
  - Keep MiniMax usage helper pure/dev-only with no live calls in unit tests.
  - Expose sanitized percentages and counts only, not exact quota totals or remaining counts.
  - Resolve `MINIMAX_API_KEY` as canonical while treating `ANTHROPIC_API_KEY` and `OPENAI_API_KEY` as aliases for the same value unless distinct values are explicitly present.
duration: 
verification_result: passed
completed_at: 2026-05-21T06:12:05.811Z
blocker_discovered: false
---

# T01: Implemented and tested the dev-only MiniMax usage/remains helper with 9router semantics and sanitized diagnostics.

**Implemented and tested the dev-only MiniMax usage/remains helper with 9router semantics and sanitized diagnostics.**

## What Happened

Implemented `arxiv_archive.minimax_usage` with pure helper primitives for MiniMax usage/remains checks. The module resolves canonical MiniMax key aliases safely, builds the verified 9router global endpoint order with Bearer auth, parses provider `base_resp` plus `model_remains`, applies token_plan used-count vs coding_plan remaining-count semantics, and exposes only sanitized quota percentages/metadata. Tests cover endpoint order, key alias redaction, provider error handling, and count semantics.

## Verification

Fresh verification passed: `uv run pytest tests/test_minimax_usage.py -q` showed 5 passed; `uv run ruff check src/arxiv_archive/minimax_usage.py tests/test_minimax_usage.py` passed; guard assertion printed `minimax-usage-helper-guard-ok`; LSP diagnostics reported no diagnostics for source/test files.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_minimax_usage.py -q` | 0 | ✅ pass — 5 passed | 17100ms |
| 2 | `uv run ruff check src/arxiv_archive/minimax_usage.py tests/test_minimax_usage.py` | 0 | ✅ pass — All checks passed | 17100ms |
| 3 | `uv run python guard assertions` | 0 | ✅ pass — minimax-usage-helper-guard-ok | 17100ms |
| 4 | `lsp diagnostics src/arxiv_archive/minimax_usage.py and tests/test_minimax_usage.py` | 0 | ✅ pass — no diagnostics | 0ms |

## Deviations

None.

## Known Issues

No live MiniMax call is performed by this helper test suite; live callability was already proven in M016. S03 still needs structured-output helper boundary implementation.

## Files Created/Modified

- `src/arxiv_archive/minimax_usage.py`
- `tests/test_minimax_usage.py`
- `.gsd/milestones/M017-cf3fd0/slices/S02/run-evidence/minimax-usage-helper-guard.json`
