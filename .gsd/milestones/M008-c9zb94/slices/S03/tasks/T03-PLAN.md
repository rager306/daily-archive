---
estimated_steps: 1
estimated_files: 5
skills_used: []
---

# T03: Run quota-gated validation scan

Run validation-batch scan over the quota-filled S02 state and write scan/delta/outlier artifacts plus a scan report. Include quota-fill evidence and PDF caveat in the report.

## Inputs

- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-summary.json`
- `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/batch-state.json`
- `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-summary.json`

## Expected Output

- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/validation-scan-summary.json`
- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/validation-scan-diagnostics.jsonl`
- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/delta-report.json`
- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/outlier-report.json`
- `.gsd/milestones/M008-c9zb94/slices/S03/validation-scan-report.md`

## Verification

test -s .gsd/milestones/M008-c9zb94/slices/S03/run-evidence/validation-scan-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
q=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-summary.json').read_text())
s=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/validation-scan-summary.json').read_text())
assert q['accepted_ready_count']==10
assert s['paper_count']==10
assert s['aggregate']['import_eligible_chunk_count']==0
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('quota-gated-scan-ok')
PY

## Observability Impact

Scan artifacts show the first new +10 batch results and import gate status.
