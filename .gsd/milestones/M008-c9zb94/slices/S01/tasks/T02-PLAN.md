---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Selected the deterministic first new +10 corpus manifest.

Apply the deterministic selection rule to choose exactly 10 new paper IDs and write the M008 manifest plus rationale. The manifest should be compatible with validation-batch init.

## Inputs

- `.gsd/milestones/M008-c9zb94/slices/S01/run-evidence/new-plus-ten-candidate-inventory.json`

## Expected Output

- `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json`
- `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-selection-rationale.md`

## Verification

test -s .gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json && uv run python - <<'PY'
import json
from pathlib import Path
m=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json').read_text())
ids=[p['paper_id'] for p in m['papers']]
assert len(ids)==10
assert len(set(ids))==10
assert m['raw_text_included'] is False
print('new-plus-ten-manifest-ok')
PY

## Observability Impact

Manifest/rationale record deterministic selection rule and per-paper source path preview.
