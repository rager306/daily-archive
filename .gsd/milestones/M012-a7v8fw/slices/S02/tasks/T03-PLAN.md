---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Write MiniMax compatibility guard

Synthesize MiniMax findings into a compatibility guard with go/no-go, preconditions, adapter implications, and blocked orchestrator/source-of-truth behavior.

## Inputs

- `.gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-research-report.md`
- `.gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-probe.json`

## Expected Output

- `.gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-compatibility-guard.json`
- `.gsd/milestones/M012-a7v8fw/slices/S02/minimax-compatibility-summary.md`

## Verification

test -s .gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-compatibility-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-compatibility-guard.json').read_text())
assert g['production_import_attempted'] is False
assert g['minimax_orchestrator_allowed'] is False
print('minimax-compatibility-guard-ok')
PY

## Observability Impact

Guard exposes MiniMax readiness/preconditions and blocked orchestrator/import behavior.
