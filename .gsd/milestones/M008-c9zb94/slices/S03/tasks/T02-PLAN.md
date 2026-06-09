---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Wrote the M008 quota-fill artifact proving 10/10 accepted source-ready papers before scan.

Generate quota-fill summary and diagnostics for the current M008 new +10 batch from the final S02 preflight state. Because current batch is 10/10 ready, no replacements should be needed, but the artifact must prove that before scan.

## Inputs

- `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/batch-state.json`
- `.gsd/milestones/M008-c9zb94/slices/S01/run-evidence/new-plus-ten-candidate-inventory.json`

## Expected Output

- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-summary.json`
- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-diagnostics.jsonl`

## Verification

test -s .gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-summary.json').read_text())
assert s['target_count']==10
assert s['accepted_ready_count']==10
assert s['shortage_count']==0
assert s['raw_text_included'] is False
print('quota-fill-ok')
PY

## Observability Impact

Quota-fill artifact becomes the scan go/no-go gate.
