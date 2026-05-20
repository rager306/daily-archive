# S02: 9router compatible live limit probe

**Goal:** Run corrected MiniMax limit probe using 9router endpoint order and parser rules.
**Demo:** After S02, M016 has corrected MiniMax limit-check verdict using 9router algorithm.

## Must-Haves

- Corrected 9router endpoint order tested.
- Success classification matches 9router parser.
- No raw response/secrets/exact quota values persisted.
- Final verdict states whether M015 limit status changes.

## Proof Level

- This slice proves: Live probe plus JSON guard.

## Integration Closure

Updates R044 and closes M016 with corrected limit verdict.

## Verification

- Sanitized 9router-compatible live probe and final guard.

## Tasks

- [x] **T01: Run 9router-compatible limit probe** `est:small`
  Run live MiniMax limit probe using exactly the 9router endpoint order and success criteria for minimax and minimax-cn where applicable.
  - Files: `.gsd/milestones/M016-9819d1/slices/S02/run-evidence/9router-compatible-limit-probe.json`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
p=Path('.gsd/milestones/M016-9819d1/slices/S02/run-evidence/9router-compatible-limit-probe.json')
d=json.loads(p.read_text())
assert d['used_9router_endpoint_order'] is True
assert d['raw_response_persisted'] is False
assert d['credential_values_logged'] is False
assert d['success_criteria']['requires_base_resp_status_zero'] is True
print('9router-compatible-limit-probe-ok')
PY

- [x] **T02: Write M016 final limit verdict** `est:small`
  Write final M016 guard and recommendation, update R044 with corrected verdict.
  - Files: `.gsd/milestones/M016-9819d1/slices/S02/run-evidence/final-m016-guard.json`, `.gsd/milestones/M016-9819d1/slices/S02/m016-final-recommendation.md`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M016-9819d1/slices/S02/run-evidence/final-m016-guard.json').read_text())
assert g['used_9router_algorithm'] is True
assert g['raw_response_persisted'] is False
assert g['credential_values_logged'] is False
assert g['limit_check_verdict'] in {'api_remains_verified','still_blocked_with_9router_algorithm'}
print('final-m016-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M016-9819d1/slices/S02/run-evidence/9router-compatible-limit-probe.json
- .gsd/milestones/M016-9819d1/slices/S02/run-evidence/final-m016-guard.json
- .gsd/milestones/M016-9819d1/slices/S02/m016-final-recommendation.md
