---
id: T01
parent: S02
milestone: M014-65dlgp
key_files:
  - .gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-helper-probes.json
key_decisions:
  - Record schema truncation as evidence rather than hiding it.
  - Use a higher completion budget retry to separate helper feasibility from truncation behavior.
duration: 
verification_result: passed
completed_at: 2026-05-20T11:18:49.823Z
blocker_discovered: false
---

# T01: Ran four real MiniMax helper probes: HTTP 200 for all, strict JSON and redacted helper succeeded with schema-validation caveats.

**Ran four real MiniMax helper probes: HTTP 200 for all, strict JSON and redacted helper succeeded with schema-validation caveats.**

## What Happened

Ran four real MiniMax OpenAI-compatible chat completion calls under the S01 live-test envelope. All four returned HTTP 200. The strict JSON contract parsed successfully. The first redacted KG helper decision call hit `finish_reason=length` and failed schema parse, so a high-budget retry was run and produced a parseable redacted helper result. The deliberate low-token edge recorded expected schema failure. Artifacts persist only hashes, statuses, usage metadata, parsed keys/booleans, and safe normalized labels; they do not persist raw prompts, raw response bodies, raw model content, secrets, raw paper/chunk text, embeddings, or vectors.

## Verification

minimax-real-helper-probes-ok and minimax-redacted-helper-retry-ok passed. live_call_count=4, successful_http_count=4, json_parse_success_count=2, redacted_helper_success_count=1, raw_response_persisted=false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `MiniMax live helper probes and JSON invariant check` | 0 | ✅ pass — live_call_count=3 initially, HTTP 200 all, one JSON success, one edge recorded | 14800ms |
| 2 | `MiniMax redacted helper retry and JSON invariant check` | 0 | ✅ pass — live_call_count=4, redacted_helper_success_count=1, raw_response_persisted=false | 17200ms |

## Deviations

Planned at least three calls; ran one additional bounded retry because the first redacted helper call returned HTTP 200 but was truncated before valid JSON. Total live calls remained under the S01 cap of six.

## Known Issues

Strict JSON is possible, but schema reliability is not automatic: one helper call truncated before JSON and one deliberate length edge failed schema parsing. Future helper integration needs local schema validation and retry/fail-closed behavior.

## Files Created/Modified

- `.gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-helper-probes.json`
