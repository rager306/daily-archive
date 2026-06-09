---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Recorded real scan provenance and verified the corrected M010 scan artifacts as fresh.

Create a real scan provenance JSONL entry for the S03 scan inputs/outputs with expected milestone_id and batch_id metadata, then run verify-artifacts and persist freshness report.

## Inputs

- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/validation-scan-summary.json`

## Expected Output

- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-provenance.jsonl`
- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-freshness-report.json`

## Verification

test -s .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-freshness-report.json && uv run python - <<'PY'
import json
from pathlib import Path
r=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-freshness-report.json').read_text())
assert r['verdict']=='fresh'
print('scan-provenance-fresh-ok')
PY

## Observability Impact

Proves scan artifacts are fresh from the recorded run context and lineage metadata matches.
