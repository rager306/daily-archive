---
id: T02
parent: S01
milestone: M014-65dlgp
key_files:
  - .gsd/milestones/M014-65dlgp/slices/S01/run-evidence/token-plan-remains-probe.json
key_decisions:
  - Persist only status/shape/hash for remains endpoint; do not persist raw body or usage values.
  - Treat 403 as likely current key not being a Token Plan Key or not authorized for Token Plan remains.
duration: 
verification_result: passed
completed_at: 2026-05-20T11:12:45.409Z
blocker_discovered: false
---

# T02: Probed Token Plan remains endpoint safely; current key returned HTTP 403 with raw response redacted.

**Probed Token Plan remains endpoint safely; current key returned HTTP 403 with raw response redacted.**

## What Happened

Called the documented MiniMax Token Plan remains endpoint using the available MiniMax key. The request was attempted, returned HTTP 403, and the artifact records only status, hashes, field-shape metadata, and redaction flags. It does not persist raw body, exact usage values, or any credential value.

## Verification

token-plan-remains-probe-ok confirmed credential_value_logged=false, raw_response_persisted=false, and endpoint suffix.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `GET https://www.minimax.io/v1/token_plan/remains with available env key, persist sanitized metadata only` | 0 | ✅ pass — HTTP 403 captured safely; raw_response_persisted=false | 8100ms |
| 2 | `JSON invariant check for token-plan-remains-probe.json` | 0 | ✅ pass — token-plan-remains-probe-ok | 8100ms |

## Deviations

The endpoint was reachable but returned HTTP 403 with the currently available key. This is recorded as access/credential-type evidence, not a failure of the documented usage method.

## Known Issues

The current environment key can call chat completions but did not authorize `/v1/token_plan/remains` (HTTP 403). To see live plan remains via API, use the actual Token Plan Key documented in Billing > Token Plan.

## Files Created/Modified

- `.gsd/milestones/M014-65dlgp/slices/S01/run-evidence/token-plan-remains-probe.json`
