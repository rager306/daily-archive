---
estimated_steps: 1
estimated_files: 4
skills_used: []
---

# T03: Refresh final preflight and report readiness

Rerun validation-batch preflight after bounded acquisition, write final preflight artifacts and source readiness report. If any Markdown remains missing, mark S03 blocked rather than scanning.

## Inputs

- `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-acquisition-summary.json`

## Expected Output

- `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/batch-state.json`
- `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-preflight-summary.json`
- `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-preflight-diagnostics.jsonl`
- `.gsd/milestones/M008-c9zb94/slices/S02/source-preflight-report.md`

## Verification

test -s .gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-preflight-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-preflight-summary.json').read_text())
assert s['paper_count']==10
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('final-preflight-ok')
PY

## Observability Impact

Final preflight report becomes the S03 go/block input.
