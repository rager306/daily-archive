---
estimated_steps: 1
estimated_files: 5
skills_used: []
---

# T03: Ran the M007 validation-batch scan dry run and produced scan/delta/outlier evidence.

Run bounded scan dry run over the S02 30-paper batch state and write M007 scan/delta/outlier artifacts plus a short report. Verify zero import eligibility, no production writes, and expected 4,289 chunks.

## Inputs

- `.gsd/milestones/M007-opaont/slices/S02/run-evidence/batch-state.json`
- `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-summary.json`

## Expected Output

- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-summary.json`
- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-diagnostics.jsonl`
- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/delta-report.json`
- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/outlier-report.json`
- `.gsd/milestones/M007-opaont/slices/S03/validation-scan-report.md`

## Verification

test -s .gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-summary.json').read_text())
assert s['paper_count']==30
assert s['aggregate']['chunk_count']==4289
assert s['aggregate']['import_eligible_chunk_count']==0
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('validation-scan-ok')
PY

## Observability Impact

Produces real M007 scan/delta/outlier evidence for S04 review.
