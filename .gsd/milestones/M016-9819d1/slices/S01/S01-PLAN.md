# S01: S01

**Goal:** Extract 9router MiniMax usage/remains implementation and tests into source-backed report.
**Demo:** After S01, we know exactly how 9router checks MiniMax limits.

## Must-Haves

- Exact endpoint order documented.
- Method/headers documented.
- base_resp status handling documented.
- model_remains parsing and count semantics documented.
- M015 endpoint omission identified.

## Proof Level

- This slice proves: Source and test evidence from indexed 9router repo.

## Integration Closure

Feeds S02 corrected probe.

## Verification

- Adds 9router algorithm report and machine-readable summary.

## Tasks

- [x] **T01: Documented 9router’s MiniMax usage algorithm and found the exact M015 endpoint omission.** `est:small`
  Write source-backed report and JSON summary for 9router MiniMax usage implementation.
  - Files: `.gsd/milestones/M016-9819d1/slices/S01/9router-minimax-usage-report.md`, `.gsd/milestones/M016-9819d1/slices/S01/run-evidence/9router-minimax-usage-summary.json`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M016-9819d1/slices/S01/run-evidence/9router-minimax-usage-summary.json').read_text())
assert s['gitnexus_repo']=='9router'
assert 'https://api.minimax.io/v1/api/openplatform/coding_plan/remains' in s['endpoint_order']['minimax']
assert s['success_requires_base_resp_status_zero'] is True
print('9router-minimax-usage-summary-ok')
PY

## Files Likely Touched

- .gsd/milestones/M016-9819d1/slices/S01/9router-minimax-usage-report.md
- .gsd/milestones/M016-9819d1/slices/S01/run-evidence/9router-minimax-usage-summary.json
