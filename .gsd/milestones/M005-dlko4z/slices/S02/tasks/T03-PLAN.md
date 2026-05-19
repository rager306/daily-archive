---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Create baseline review samples

Generate bounded review samples for the six-paper inner review minimum where artifacts are available, and explicit blocker records where they are not. Review samples may include bounded snippets only in markdown review artifacts; machine JSON/JSONL diagnostics must remain redacted.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-package-diagnostics.jsonl`
- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json`

## Expected Output

- `.gsd/milestones/M005-dlko4z/slices/S02/review/baseline-review-samples.md`
- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/review-sample-index.json`

## Verification

test -s .gsd/milestones/M005-dlko4z/slices/S02/review/baseline-review-samples.md && uv run python - <<'PY'
import json
from pathlib import Path
p=Path('.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/review-sample-index.json')
data=json.loads(p.read_text())
assert data['schema_version']=='m005-baseline-review-sample-index.v1'
assert data['raw_text_in_machine_logs'] is False
PY

## Observability Impact

Separates human-readable bounded snippets from redacted machine logs and indexes sample coverage/blockers.
