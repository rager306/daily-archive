---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Bounded acquisition made the new +10 batch 10/10 Markdown-ready via arxiv2md.

If initial preflight is not source-ready, run bounded Markdown acquisition over the new +10 manifest using existing source acquisition helper. Prefer fast arxiv2md first; do not run unbounded conversion loops. Persist acquisition summary and diagnostics.

## Inputs

- `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json`

## Expected Output

- `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-acquisition-summary.json`
- `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-acquisition-diagnostics.jsonl`

## Verification

test -s .gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-acquisition-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-acquisition-summary.json').read_text())
assert s['paper_count']==10
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('source-acquisition-ok')
PY

## Observability Impact

Acquisition artifacts show attempted methods, outcomes, and remaining blockers without raw text.
