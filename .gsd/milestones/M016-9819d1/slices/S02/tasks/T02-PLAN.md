---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Final M016 guard verifies global MiniMax API remains through the 9router fallback endpoint and overturns M015's limit verdict.

Write final M016 guard and recommendation, update R044 with corrected verdict.

## Inputs

- `.gsd/milestones/M016-9819d1/slices/S02/run-evidence/9router-compatible-limit-probe.json`

## Expected Output

- `.gsd/milestones/M016-9819d1/slices/S02/run-evidence/final-m016-guard.json`
- `.gsd/milestones/M016-9819d1/slices/S02/m016-final-recommendation.md`

## Verification

uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M016-9819d1/slices/S02/run-evidence/final-m016-guard.json').read_text())
assert g['used_9router_algorithm'] is True
assert g['raw_response_persisted'] is False
assert g['credential_values_logged'] is False
assert g['limit_check_verdict'] in {'api_remains_verified','still_blocked_with_9router_algorithm'}
print('final-m016-guard-ok')
PY

## Observability Impact

Final corrected limit verdict.
