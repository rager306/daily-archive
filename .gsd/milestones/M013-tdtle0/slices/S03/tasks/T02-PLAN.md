---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T02: Write MiniMax smoke-test guard

Write MiniMax smoke-test guard with callability status, schema validation status, and blocked behaviors.

## Inputs

- `.gsd/milestones/M013-tdtle0/slices/S03/run-evidence/minimax-smoke-test.json`

## Expected Output

- `.gsd/milestones/M013-tdtle0/slices/S03/run-evidence/minimax-smoke-test-guard.json`

## Verification

test -s .gsd/milestones/M013-tdtle0/slices/S03/run-evidence/minimax-smoke-test-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M013-tdtle0/slices/S03/run-evidence/minimax-smoke-test-guard.json').read_text())
assert g['secrets_logged'] is False
assert g['minimax_orchestrator_allowed'] is False
print('minimax-smoke-test-guard-ok')
PY

## Observability Impact

Guard feeds final recommendation.
