---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T03: Final S04 guard passed and records the FLAG review plus next-batch top-up requirement.

Run final artifact guards for M008 S04 and milestone-ready status: quota accepted count, scan count, import gate, no-write/no-import flags, and review/recommendation presence.

## Inputs

- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-summary.json`
- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/validation-scan-summary.json`
- `.gsd/milestones/M008-c9zb94/slices/S04/run-evidence/new-plus-ten-review-summary.md`
- `.gsd/milestones/M008-c9zb94/slices/S04/new-plus-ten-final-recommendation.md`

## Expected Output

- `.gsd/milestones/M008-c9zb94/slices/S04/run-evidence/final-review-guard.json`

## Verification

test -s .gsd/milestones/M008-c9zb94/slices/S04/run-evidence/final-review-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S04/run-evidence/final-review-guard.json').read_text())
assert g['quota_ready']==10
assert g['paper_count']==10
assert g['import_eligible_chunk_count']==0
assert g['production_import_attempted'] is False
assert g['ladybugdb_written'] is False
print('final-review-guard-ok')
PY

## Observability Impact

Machine-readable final guard summarizes whether M008 can move to validation/close.
