# S02: MiniMax usage limit helper

**Goal:** Implement sanitized MiniMax usage/remains helper using M016/9router semantics.
**Demo:** After S02, the project has a tested dev-only MiniMax limit helper contract.

## Must-Haves

- Canonical MINIMAX_API_KEY alias mapping preserved.
- Endpoint order and provider status rules implemented.
- token_plan vs coding_plan count semantics tested.
- Raw responses/exact quotas/secrets not persisted.

## Proof Level

- This slice proves: Unit tests over sanitized fixtures and no-secret assertions.

## Integration Closure

Provides limit observability for bounded helper usage; does not call MiniMax live in unit tests.

## Verification

- Adds redacted limit-check diagnostics and fixture coverage.

## Tasks

- [x] **T01: Implement MiniMax usage limit helper** `est:medium`
  Implement a dev-only MiniMax usage/remains helper module with pure parsing/request-building primitives and sanitized summaries. Cover 9router endpoint order, provider status success, token_plan used-count semantics, coding_plan remaining-count semantics, key alias mapping, and no raw response/exact quota/secret persistence with tests.
  - Files: `src/arxiv_archive/minimax_usage.py`, `tests/test_minimax_usage.py`, `.gsd/milestones/M017-cf3fd0/slices/S02/run-evidence/minimax-usage-helper-guard.json`
  - Verify: uv run pytest tests/test_minimax_usage.py -q && uv run ruff check src/arxiv_archive/minimax_usage.py tests/test_minimax_usage.py && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M017-cf3fd0/slices/S02/run-evidence/minimax-usage-helper-guard.json').read_text())
assert g['raw_response_persisted'] is False
assert g['exact_quota_values_persisted'] is False
assert g['credential_values_logged'] is False
assert g['production_import_allowed'] is False
print('minimax-usage-helper-guard-ok')
PY

## Files Likely Touched

- src/arxiv_archive/minimax_usage.py
- tests/test_minimax_usage.py
- .gsd/milestones/M017-cf3fd0/slices/S02/run-evidence/minimax-usage-helper-guard.json
