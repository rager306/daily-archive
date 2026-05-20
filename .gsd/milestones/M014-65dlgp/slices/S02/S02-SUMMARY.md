---
id: S02
parent: M014-65dlgp
milestone: M014-65dlgp
provides:
  - MiniMax real-test guard
  - Helper integration probe recommendation input
requires:
  - slice: S01
    provides: Live-call cap, payload policy, and Token Plan context.
affects:
  - S03
key_files:
  - .gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-test-guard.json
key_decisions:
  - MiniMax can be used for bounded redacted helper probes if local schema validation and bounded retry are mandatory.
  - MiniMax schema adherence is not reliable enough for source-of-truth use.
  - Raw responses/model content must never be persisted, even for real tests.
patterns_established:
  - Real MiniMax helper calls require local schema validation and bounded retry on length/truncation.
  - Persist response hashes/metadata, not raw model content.
observability_surfaces:
  - per-call status/hash/usage metadata
  - schema parse booleans
  - real-test guard
drill_down_paths:
  - .gsd/milestones/M014-65dlgp/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M014-65dlgp/slices/S02/tasks/T02-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-20T11:20:14.114Z
blocker_discovered: false
---

# S02: MiniMax real bounded helper probes

**S02 proved MiniMax real helper callability under bounded redacted inputs, with schema validation caveats.**

## What Happened

S02 ran four real MiniMax OpenAI-compatible chat completion calls. All returned HTTP 200. Strict JSON succeeded. The first redacted helper probe truncated and failed schema parse; a higher-budget retry succeeded. A deliberate low-token edge recorded fail-closed schema behavior. The guard permits only next helper integration probe with local schema validation and retry controls, while blocking unattended batch use, source-of-truth use, production import, LadybugDB writes, and orchestration.

## Verification

minimax-real-helper-probes-ok, minimax-redacted-helper-retry-ok, and minimax-real-test-guard-ok passed.

## Requirements Advanced

- R042 — S02 validates the real bounded API test portion of R042.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

Ran four live calls instead of three because the first redacted helper-style call returned HTTP 200 but was truncated before valid JSON. The extra retry stayed within S01 cap of six calls and proved helper feasibility under higher completion budget.

## Known Limitations

All calls used synthetic/redacted metadata only. No raw project/paper/chunk text was sent. This proves helper callability, not scientific correctness.

## Follow-ups

S03 should independently review the evidence and recommend only a schema-validated helper integration probe, not source-of-truth or production use.

## Files Created/Modified

- `.gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-helper-probes.json` — Sanitized real MiniMax helper probe results.
- `.gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-test-guard.json` — S02 real-test guard.
