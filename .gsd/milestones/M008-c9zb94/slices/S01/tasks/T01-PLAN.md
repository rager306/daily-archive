---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Build candidate inventory

Survey existing local research/cache inventory (`/root/.research/papers` and `/root/.arxiv_cache`) plus the M006 manifest to identify candidate paper IDs not already in the 30-paper corpus. Produce a compact candidate inventory summary without raw text.

## Inputs

- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json`

## Expected Output

- `.gsd/milestones/M008-c9zb94/slices/S01/run-evidence/new-plus-ten-candidate-inventory.json`

## Verification

test -s .gsd/milestones/M008-c9zb94/slices/S01/run-evidence/new-plus-ten-candidate-inventory.json && uv run python - <<'PY'
import json
from pathlib import Path
p=Path('.gsd/milestones/M008-c9zb94/slices/S01/run-evidence/new-plus-ten-candidate-inventory.json')
s=json.loads(p.read_text())
assert s['candidate_count'] >= 10
assert s['raw_text_included'] is False
print('candidate-inventory-ok')
PY

## Observability Impact

Candidate inventory shows counts and redacted path availability for selection.
