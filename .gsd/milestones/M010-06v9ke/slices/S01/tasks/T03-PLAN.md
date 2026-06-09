---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Wrote the M010 availability report and guard: 10 selected, 0 prior overlap, 0/10 upfront Markdown/PDF.

Write availability report and run final S01 guards: selected count, overlap count, source availability counts, and safety flags.

## Inputs

- `.gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-corpus-manifest.json`

## Expected Output

- `.gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-availability-report.md`
- `.gsd/milestones/M010-06v9ke/slices/S01/run-evidence/selection-guard.json`

## Verification

test -s .gsd/milestones/M010-06v9ke/slices/S01/run-evidence/selection-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S01/run-evidence/selection-guard.json').read_text())
assert g['selected_count']==10
assert g['prior_overlap_count']==0
assert g['raw_text_included'] is False
print('selection-guard-ok')
PY

## Observability Impact

Selection guard becomes S02 input proof.
