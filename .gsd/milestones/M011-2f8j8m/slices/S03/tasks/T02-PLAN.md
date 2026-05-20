---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T02: Write independent review guard

Write a review guard that captures verdict, scope, leakage status, and whether positive import remains blocked.

## Inputs

- `.gsd/milestones/M011-2f8j8m/slices/S03/run-evidence/semantic-gate-independent-review.md`

## Expected Output

- `.gsd/milestones/M011-2f8j8m/slices/S03/run-evidence/semantic-review-guard.json`

## Verification

test -s .gsd/milestones/M011-2f8j8m/slices/S03/run-evidence/semantic-review-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M011-2f8j8m/slices/S03/run-evidence/semantic-review-guard.json').read_text())
assert g['review_verdict'] in {'PASS','FLAG'}
assert g['positive_import_blocked'] is True
assert g['raw_payload_key_count'] == 0
print('semantic-review-guard-ok')
PY

## Observability Impact

Review guard consolidates S03 findings for S04.
