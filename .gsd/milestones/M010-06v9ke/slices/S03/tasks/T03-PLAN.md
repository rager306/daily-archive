---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Wrote the S03 scan report and guard proving fresh active-lineage operational scan evidence.

Run final S03 scan guard across quota, scan counts, active lineage, provenance freshness, and safety flags. Write validation scan report.

## Inputs

- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-freshness-report.json`
- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/validation-scan-summary.json`

## Expected Output

- `.gsd/milestones/M010-06v9ke/slices/S03/validation-scan-report.md`
- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-guard.json`

## Verification

test -s .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-guard.json').read_text())
assert g['paper_count']==10
assert g['freshness_verdict']=='fresh'
assert g['milestone_id']=='M010-06v9ke'
assert g['production_import_attempted'] is False
assert g['ladybugdb_written'] is False
print('scan-guard-ok')
PY

## Observability Impact

Scan guard becomes S04 review input.
