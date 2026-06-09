---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Selected the M010 next +10 manifest with 0 prior overlap and 0/10 upfront Markdown/PDF availability.

Select the first 10 deterministic candidate IDs after exclusions and write the M010 manifest plus rationale.

## Inputs

- `.gsd/milestones/M010-06v9ke/slices/S01/run-evidence/next-plus-ten-candidate-inventory.json`

## Expected Output

- `.gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-corpus-manifest.json`
- `.gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-selection-rationale.md`

## Verification

test -s .gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-corpus-manifest.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-corpus-manifest.json').read_text())
assert s['paper_count']==10
assert s['prior_overlap_count']==0
assert s['raw_text_included'] is False
print('manifest-ok')
PY

## Observability Impact

Manifest/rationale explain deterministic selection without raw content.
