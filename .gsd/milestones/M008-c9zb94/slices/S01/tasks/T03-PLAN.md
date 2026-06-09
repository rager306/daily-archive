---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T03: Audited the new +10 manifest: no M006 overlap, 1/10 Markdown-ready before S02.

Run an overlap/source preview guard against M006 corpus and write a short availability report. Confirm no overlap and no raw text leakage.

## Inputs

- `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json`
- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json`

## Expected Output

- `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-availability-report.md`

## Verification

test -s .gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-availability-report.md && uv run python - <<'PY'
import json
from pathlib import Path
m=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json').read_text())
old=json.loads(Path('.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json').read_text())
ids={p['paper_id'] for p in m['papers']}
old_ids={p['paper_id'] for p in old['papers']}
assert not ids & old_ids
print('overlap-audit-ok')
PY

## Observability Impact

Availability report makes S02 expectations explicit before preflight.
