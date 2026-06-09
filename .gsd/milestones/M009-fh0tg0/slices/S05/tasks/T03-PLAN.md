---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T03: Final hardening guard passed with FLAG review and explicit next-batch gates.

Run final guard across provenance, verifier, lineage, and top-up artifacts plus focused tests.

## Inputs

- `.gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-pass-report.json`
- `.gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-stale-report.json`
- `.gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-mismatch-report.json`
- `.gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-pass-summary.json`
- `.gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-blocked-summary.json`

## Expected Output

- `.gsd/milestones/M009-fh0tg0/slices/S05/run-evidence/final-hardening-guard.json`

## Verification

test -s .gsd/milestones/M009-fh0tg0/slices/S05/run-evidence/final-hardening-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M009-fh0tg0/slices/S05/run-evidence/final-hardening-guard.json').read_text())
assert g['freshness_pass_verdict']=='fresh'
assert g['freshness_stale_verdict']=='stale'
assert g['lineage_mismatch_verdict']=='stale'
assert g['top_up_pass_scan_allowed'] is True
assert g['top_up_blocked_scan_allowed'] is False
print('final-hardening-guard-ok')
PY

## Observability Impact

Final guard summarizes whether M009 can close and what the next +10 requires.
