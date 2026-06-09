---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T02: Wrote integration guard blocking production activation and naming the next safe probes.

Write a failure-mode and activation-precondition guard that proves DSPy and MiniMax remain disabled in the production process and identifies exact next probes.

## Inputs

- `.gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-matrix.json`

## Expected Output

- `.gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-guard.json`

## Verification

test -s .gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-guard.json').read_text())
assert g['dspy_production_runtime_allowed'] is False
assert g['minimax_orchestrator_allowed'] is False
assert g['production_import_allowed'] is False
print('integration-guard-ok')
PY

## Observability Impact

Guard records blocked surfaces and next probe preconditions.
