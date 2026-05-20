---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T03: Verify judgment consistency and leakage guard

Run consistency and leakage guard over the rubric and judgments, including class counts, blocker counts, and no-write/no-import safety flags.

## Inputs

- `.gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/redacted-semantic-judgments.json`

## Expected Output

- `.gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/semantic-judgment-guard.json`

## Verification

test -s .gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/semantic-judgment-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/semantic-judgment-guard.json').read_text())
assert g['target_count'] == 10
assert g['raw_payload_key_count'] == 0
assert g['positive_import_recommended'] is False
print('semantic-judgment-guard-ok')
PY

## Observability Impact

Guard is S03 independent review input.
