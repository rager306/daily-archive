---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T01: Initialized and preflighted the new +10 batch; only 1/10 is initially Markdown-ready.

Run validation-batch init and initial preflight against the M008 new +10 manifest. Persist init/preflight responses and summarize initial readiness without acquisition.

## Inputs

- `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json`

## Expected Output

- `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/init-response.json`
- `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/initial-source-preflight-summary.json`
- `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/initial-source-preflight-diagnostics.jsonl`

## Verification

test -s .gsd/milestones/M008-c9zb94/slices/S02/run-evidence/initial-source-preflight-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/initial-source-preflight-summary.json').read_text())
assert s['paper_count']==10
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('initial-preflight-ok')
PY

## Observability Impact

Initial preflight captures real source gaps before any repair/acquisition.
