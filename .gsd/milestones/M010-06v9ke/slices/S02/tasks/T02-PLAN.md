---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Run bounded M010 source acquisition

Run bounded fast-only source acquisition for selected M010 papers. Persist acquisition summary/diagnostics; do not use unbounded conversion.

## Inputs

- `.gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-corpus-manifest.json`

## Expected Output

- `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-acquisition-summary.json`
- `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-acquisition-diagnostics.jsonl`

## Verification

test -s .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-acquisition-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-acquisition-summary.json').read_text())
assert s['paper_count']==10
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('source-acquisition-ok')
PY

## Observability Impact

Acquisition evidence shows attempted/acquired/missing counts without raw text.
