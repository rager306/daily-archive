---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T03: Write optimizer applicability guard

Write optimizer guard proving no optimizer was run and summarizing which optimizer families are future-only versus blocked.

## Inputs

- `.gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-applicability.json`

## Expected Output

- `.gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-guard.json`

## Verification

test -s .gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-guard.json').read_text())
assert g['optimizer_executed'] is False
assert g['production_import_allowed'] is False
print('dspy-optimizer-guard-ok')
PY

## Observability Impact

Guard feeds final recommendation.
