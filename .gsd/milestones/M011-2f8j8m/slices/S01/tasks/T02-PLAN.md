---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Built the M011 semantic review target set: 10 redacted targets with source paths and hashes.

Build deterministic redacted review-set manifest with a bounded mix of M010 outliers and non-outlier controls, carrying source path/hash/span metadata only.

## Inputs

- `.gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/schema-inspection.json`

## Expected Output

- `.gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/semantic-review-targets.json`
- `.gsd/milestones/M011-2f8j8m/slices/S01/semantic-review-selection-rationale.md`

## Verification

test -s .gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/semantic-review-targets.json && uv run python - <<'PY'
import json
from pathlib import Path
m=json.loads(Path('.gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/semantic-review-targets.json').read_text())
assert m['target_count'] > 0
assert m['raw_text_included'] is False
assert m['chunk_text_included'] is False
print('semantic-review-targets-ok')
PY

## Observability Impact

Provides deterministic review corpus for S02.
