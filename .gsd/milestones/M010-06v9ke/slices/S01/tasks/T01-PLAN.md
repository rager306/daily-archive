---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Built the M010 candidate inventory: 790 eligible papers after excluding 40 prior validation IDs.

Build prior-corpus exclusion set from M006 and M008 manifests and a redacted candidate inventory from local/cache paper metadata. Do not include raw text.

## Inputs

- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json`
- `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json`

## Expected Output

- `.gsd/milestones/M010-06v9ke/slices/S01/run-evidence/next-plus-ten-candidate-inventory.json`

## Verification

test -s .gsd/milestones/M010-06v9ke/slices/S01/run-evidence/next-plus-ten-candidate-inventory.json && uv run python - <<'PY'
import json
from pathlib import Path
p=Path('.gsd/milestones/M010-06v9ke/slices/S01/run-evidence/next-plus-ten-candidate-inventory.json')
s=json.loads(p.read_text())
assert s['candidate_count'] >= 10
assert s['raw_text_included'] is False
print('candidate-inventory-ok')
PY

## Observability Impact

Inventory records candidate IDs and source availability only.
