---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T01: Implement MiniMax usage limit helper

Implement a dev-only MiniMax usage/remains helper module with pure parsing/request-building primitives and sanitized summaries. Cover 9router endpoint order, provider status success, token_plan used-count semantics, coding_plan remaining-count semantics, key alias mapping, and no raw response/exact quota/secret persistence with tests.

## Inputs

- `.gsd/milestones/M016-9819d1/slices/S02/run-evidence/final-m016-guard.json`

## Expected Output

- `src/arxiv_archive/minimax_usage.py`
- `tests/test_minimax_usage.py`
- `.gsd/milestones/M017-cf3fd0/slices/S02/run-evidence/minimax-usage-helper-guard.json`

## Verification

uv run pytest tests/test_minimax_usage.py -q && uv run ruff check src/arxiv_archive/minimax_usage.py tests/test_minimax_usage.py && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M017-cf3fd0/slices/S02/run-evidence/minimax-usage-helper-guard.json').read_text())
assert g['raw_response_persisted'] is False
assert g['exact_quota_values_persisted'] is False
assert g['credential_values_logged'] is False
assert g['production_import_allowed'] is False
print('minimax-usage-helper-guard-ok')
PY

## Observability Impact

Sanitized helper guard records safe defaults and blocked persistence.
