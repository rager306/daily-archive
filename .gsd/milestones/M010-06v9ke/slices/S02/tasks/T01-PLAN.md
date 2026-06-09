---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T01: Initialized and preflighted M010 next +10; initial readiness is 0/10.

Run validation-batch init and initial preflight for M010 S01 manifest. Persist responses and initial preflight artifacts.

## Inputs

- `.gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-corpus-manifest.json`

## Expected Output

- `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/init-response.json`
- `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/initial-source-preflight-summary.json`
- `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/initial-source-preflight-diagnostics.jsonl`

## Verification

test -s .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/initial-source-preflight-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/initial-source-preflight-summary.json').read_text())
assert s['paper_count']==10
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('initial-preflight-ok')
PY

## Observability Impact

Captures real initial source readiness before acquisition/top-up.
