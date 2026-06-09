# S01: S01

**Goal:** Debug MiniMax Token Plan remains access with available ordinary and Token Plan keys across endpoint/header/method variants.
**Demo:** After S01, we know which key/header/method combinations work or fail for Token Plan remains and whether a Token Plan Key is needed.

## Must-Haves

- Available key types are tested without logging values.
- Endpoint/header/method matrix covers documented and plausible variants.
- Raw responses and exact usage values are not persisted.
- Final S01 guard states exact method that works, or exact evidence-backed blocker.

## Proof Level

- This slice proves: Live endpoint matrix plus docs references.

## Integration Closure

Provides corrected limit-check verdict to S03.

## Verification

- Sanitized endpoint matrix with no raw bodies/secrets.

## Tasks

- [x] **T01: Ran Token Plan remains matrix; no true API remains success, and collected Token Plan key matched the ordinary API key.** `est:medium`
  Run sanitized Token Plan remains endpoint matrix across available key variables, GET/POST, Bearer/X-Api-Key, and minimax/minimaxi host variants where safe.
  - Files: `.gsd/milestones/M015-ktorc7/slices/S01/run-evidence/token-plan-access-matrix.json`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
d=json.loads(Path('.gsd/milestones/M015-ktorc7/slices/S01/run-evidence/token-plan-access-matrix.json').read_text())
assert d['raw_response_persisted'] is False
assert d['credential_values_logged'] is False
assert d['matrix_count'] >= 8
print('token-plan-access-matrix-ok')
PY

- [x] **T02: Wrote corrected Token Plan access verdict: UI works; API remains is still unverified with available key material.** `est:small`
  Write Token Plan access verdict explaining whether API-based remains checking is now proven and how user should check limits.
  - Files: `.gsd/milestones/M015-ktorc7/slices/S01/run-evidence/token-plan-access-guard.json`, `.gsd/milestones/M015-ktorc7/slices/S01/token-plan-access-remediation.md`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M015-ktorc7/slices/S01/run-evidence/token-plan-access-guard.json').read_text())
assert g['raw_response_persisted'] is False
assert g['credential_values_logged'] is False
assert g['limit_check_verdict'] in {'api_remains_verified','ui_only_or_session_required','blocked_missing_authorized_key','mixed'}
print('token-plan-access-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M015-ktorc7/slices/S01/run-evidence/token-plan-access-matrix.json
- .gsd/milestones/M015-ktorc7/slices/S01/run-evidence/token-plan-access-guard.json
- .gsd/milestones/M015-ktorc7/slices/S01/token-plan-access-remediation.md
