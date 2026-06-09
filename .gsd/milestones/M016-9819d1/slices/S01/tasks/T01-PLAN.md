---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Documented 9router’s MiniMax usage algorithm and found the exact M015 endpoint omission.

Write source-backed report and JSON summary for 9router MiniMax usage implementation.

## Inputs

- `../vendor-source/9router/open-sse/services/usage.js`
- `../vendor-source/9router/tests/unit/minimax-usage.test.js`

## Expected Output

- `.gsd/milestones/M016-9819d1/slices/S01/9router-minimax-usage-report.md`
- `.gsd/milestones/M016-9819d1/slices/S01/run-evidence/9router-minimax-usage-summary.json`

## Verification

uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M016-9819d1/slices/S01/run-evidence/9router-minimax-usage-summary.json').read_text())
assert s['gitnexus_repo']=='9router'
assert 'https://api.minimax.io/v1/api/openplatform/coding_plan/remains' in s['endpoint_order']['minimax']
assert s['success_requires_base_resp_status_zero'] is True
print('9router-minimax-usage-summary-ok')
PY

## Observability Impact

Algorithm summary for corrected S02 probe.
