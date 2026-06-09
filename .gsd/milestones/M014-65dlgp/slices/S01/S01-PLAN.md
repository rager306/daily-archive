# S01: S01

**Goal:** Document MiniMax Token Plan quotas, rate limits, subscription budget posture, and usage visibility mechanisms.
**Demo:** After S01, we know where and how to inspect Token Plan quotas/limits and current operational rules.

## Must-Haves

- Token Plan quotas by tier and reset model documented.
- Usage visibility documented: Billing > Token Plan and `/v1/token_plan/remains` endpoint.
- API rate limits documented.
- Token Plan production-suitability caveat documented.
- User budget posture captured: subscription means budget is not current blocker, but platform limits still apply.

## Proof Level

- This slice proves: Docs-backed report and guarded live endpoint check.

## Integration Closure

S01 feeds S02 with safe test envelope and S03 with limit-operability evidence.

## Verification

- Quota docs report plus optional remains endpoint metadata artifact.

## Tasks

- [x] **T01: Documented MiniMax Token Plan quotas, usage visibility, and rate-limit caveats.** `est:small`
  Use current MiniMax docs to write Token Plan quota/limit report, including usage page, remains endpoint, quota tiers, rate limits, reset rules, and production caveat.
  - Files: `.gsd/milestones/M014-65dlgp/slices/S01/token-plan-limits-report.md`, `.gsd/milestones/M014-65dlgp/slices/S01/run-evidence/token-plan-docs-summary.json`
  - Verify: test -s .gsd/milestones/M014-65dlgp/slices/S01/token-plan-limits-report.md && test -s .gsd/milestones/M014-65dlgp/slices/S01/run-evidence/token-plan-docs-summary.json

- [x] **T02: Probed Token Plan remains endpoint safely; current key returned HTTP 403 with raw response redacted.** `est:small`
  Call MiniMax Token Plan remains endpoint if the existing key can access it, persist only sanitized response shape/status/keys and no token values or raw body.
  - Files: `.gsd/milestones/M014-65dlgp/slices/S01/run-evidence/token-plan-remains-probe.json`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
p=Path('.gsd/milestones/M014-65dlgp/slices/S01/run-evidence/token-plan-remains-probe.json')
d=json.loads(p.read_text())
assert d['credential_value_logged'] is False
assert d['raw_response_persisted'] is False
assert d['endpoint'].endswith('/v1/token_plan/remains')
print('token-plan-remains-probe-ok')
PY

- [x] **T03: Wrote Token Plan limits guard: budget non-blocking, platform limits still apply, S02 capped to bounded calls.** `est:small`
  Synthesize S01 guard: budget non-blocking due subscription, platform limits still respected, and real test envelope for S02.
  - Files: `.gsd/milestones/M014-65dlgp/slices/S01/run-evidence/token-plan-limits-guard.json`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M014-65dlgp/slices/S01/run-evidence/token-plan-limits-guard.json').read_text())
assert g['subscription_budget_non_blocking'] is True
assert g['platform_limits_still_apply'] is True
assert g['raw_response_persisted'] is False
print('token-plan-limits-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M014-65dlgp/slices/S01/token-plan-limits-report.md
- .gsd/milestones/M014-65dlgp/slices/S01/run-evidence/token-plan-docs-summary.json
- .gsd/milestones/M014-65dlgp/slices/S01/run-evidence/token-plan-remains-probe.json
- .gsd/milestones/M014-65dlgp/slices/S01/run-evidence/token-plan-limits-guard.json
