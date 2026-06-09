---
estimated_steps: 1
estimated_files: 4
skills_used: []
---

# T03: Materialized two acquired replacements and produced a final 10/10 source-ready M010 batch state.

Rerun final preflight, build quota-fill evidence, and if quota remains short produce bounded top-up plan/blocker. S03 may proceed only if final source-ready quota is 10.

## Inputs

- `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-acquisition-summary.json`
- `.gsd/milestones/M010-06v9ke/slices/S01/run-evidence/next-plus-ten-candidate-inventory.json`

## Expected Output

- `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-preflight-summary.json`
- `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/quota-fill-summary.json`
- `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/top-up-summary.json`
- `.gsd/milestones/M010-06v9ke/slices/S02/source-readiness-report.md`

## Verification

test -s .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/quota-fill-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
q=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/quota-fill-summary.json').read_text())
assert q['target_count']==10
assert q['raw_text_included'] is False
print('quota-gate-ok')
PY

## Observability Impact

Final source readiness report becomes S03 go/block gate.
