---
id: T02
parent: S02
milestone: M021-xcfj4p
key_files:
  - src/arxiv_archive/candidate_locators.py
key_decisions: []
duration: 
verification_result: passed
completed_at: 2026-05-21T10:18:58.001Z
blocker_discovered: false
---

# T02: Implemented deterministic candidate locator module.

**Implemented deterministic candidate locator module.**

## What Happened

Implemented `candidate_locators.py` as an additive deterministic module. It defines locator protocol constants, source and route dataclasses, default route specs, safety flags, artifact builder, recursive forbidden-key detection, artifact validation, and safe writer. It reads source files only to compute hashes and coordinates, and does not serialize raw text.

## Verification

Final focused verification passed with pytest, ruff, and LSP diagnostics on changed files.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_candidate_locators.py -q && uv run ruff check src/arxiv_archive/candidate_locators.py tests/test_candidate_locators.py` | 0 | ✅ pass: 8 passed; All checks passed! | 9400ms |
| 2 | `lsp diagnostics src/arxiv_archive/candidate_locators.py` | 0 | ✅ pass: No diagnostics | 0ms |
| 3 | `lsp diagnostics tests/test_candidate_locators.py` | 0 | ✅ pass: No diagnostics | 0ms |

## Deviations

None.

## Known Issues

Module is callable/pure; CLI integration is not implemented in S02 and remains S03 scope.

## Files Created/Modified

- `src/arxiv_archive/candidate_locators.py`
