---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T02: Write MiniMax real-test guard

Write a real-test guard that summarizes pass/flag outcomes, schema reliability, redaction hygiene, and blocked scopes.

## Inputs

- `.gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-helper-probes.json`

## Expected Output

- `.gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-test-guard.json`

## Verification

uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-test-guard.json').read_text())
assert g['live_call_count'] >= 3
assert g['raw_response_persisted'] is False
assert g['raw_model_content_persisted'] is False
assert g['minimax_orchestrator_allowed'] is False
assert g['production_import_allowed'] is False
print('minimax-real-test-guard-ok')
PY

## Observability Impact

Provides downstream recommendation input.
