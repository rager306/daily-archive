---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Apply redacted semantic judgments

Apply the rubric to every S01 target using redacted M010 metadata and source path/hash provenance. Persist categorical judgments without raw source text or claim text.

## Inputs

- `.gsd/milestones/M011-2f8j8m/slices/S02/semantic-review-rubric.md`
- `.gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/semantic-review-targets.json`

## Expected Output

- `.gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/redacted-semantic-judgments.json`
- `.gsd/milestones/M011-2f8j8m/slices/S02/semantic-judgment-summary.md`

## Verification

test -s .gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/redacted-semantic-judgments.json && uv run python - <<'PY'
import json
from pathlib import Path
j=json.loads(Path('.gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/redacted-semantic-judgments.json').read_text())
assert j['target_count'] == 10
assert j['raw_text_included'] is False
assert j['trusted_facts_created'] is False
print('redacted-semantic-judgments-ok')
PY

## Observability Impact

Judgment packet shows why targets are or are not import-ready.
