---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T01: Ran the active-lineage M010 scan: 10 papers, 1,477 chunks, 7 outliers, zero import-eligible chunks.

Run validation-batch scan over the materialized S02 source-ready batch state with active M010 milestone lineage. Persist scan response, summary, diagnostics, delta, outlier, manifest, and report.

## Inputs

- `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-ready-batch-state.json`

## Expected Output

- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/validation-scan-summary.json`
- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/delta-report.json`
- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/outlier-report.json`

## Verification

test -s .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/validation-scan-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/validation-scan-summary.json').read_text())
assert s['paper_count']==10
assert s['milestone_id']=='M010-06v9ke'
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('active-lineage-scan-ok')
PY

## Observability Impact

Produces active-lineage scan artifacts for provenance verification.
