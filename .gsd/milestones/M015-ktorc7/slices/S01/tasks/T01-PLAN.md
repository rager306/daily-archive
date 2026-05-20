---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Run Token Plan access matrix

Run sanitized Token Plan remains endpoint matrix across available key variables, GET/POST, Bearer/X-Api-Key, and minimax/minimaxi host variants where safe.

## Inputs

- `.gsd/milestones/M014-65dlgp/slices/S01/token-plan-limits-report.md`
- `MiniMax Token Plan FAQ docs`

## Expected Output

- `.gsd/milestones/M015-ktorc7/slices/S01/run-evidence/token-plan-access-matrix.json`

## Verification

uv run python - <<'PY'
import json
from pathlib import Path
d=json.loads(Path('.gsd/milestones/M015-ktorc7/slices/S01/run-evidence/token-plan-access-matrix.json').read_text())
assert d['raw_response_persisted'] is False
assert d['credential_values_logged'] is False
assert d['matrix_count'] >= 8
print('token-plan-access-matrix-ok')
PY

## Observability Impact

Records status codes, response shape hashes, parse success, and auth mode only.
