---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Run 9router-compatible limit probe

Run live MiniMax limit probe using exactly the 9router endpoint order and success criteria for minimax and minimax-cn where applicable.

## Inputs

- `.gsd/milestones/M016-9819d1/slices/S01/run-evidence/9router-minimax-usage-summary.json`

## Expected Output

- `.gsd/milestones/M016-9819d1/slices/S02/run-evidence/9router-compatible-limit-probe.json`

## Verification

uv run python - <<'PY'
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

## Observability Impact

Corrected live endpoint evidence.
