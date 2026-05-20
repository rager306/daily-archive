---
id: M014-65dlgp
title: "MiniMax Real Test and Token Plan Limits"
status: complete
completed_at: 2026-05-20T11:29:28.575Z
key_decisions:
  - Subscription budget is non-blocking for current MiniMax tests, but platform limits still apply.
  - MiniMax is usable only as bounded helper candidate with schema validation and retry controls.
  - MiniMax remains blocked as source of truth, orchestrator, unattended batch engine, production importer, or fact promoter.
  - Token Plan weekly quota and peak-hour guidance must be treated as operational constraints.
key_files:
  - .gsd/milestones/M014-65dlgp/slices/S01/token-plan-limits-report.md
  - .gsd/milestones/M014-65dlgp/slices/S01/run-evidence/token-plan-limits-guard.json
  - .gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-helper-probes.json
  - .gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-test-guard.json
  - .gsd/milestones/M014-65dlgp/slices/S03/run-evidence/final-m014-guard.json
  - .gsd/milestones/M014-65dlgp/slices/S03/m014-final-recommendation.md
  - .gsd/milestones/M014-65dlgp/M014-65dlgp-VALIDATION.md
lessons_learned:
  - MiniMax can return HTTP 200 but still fail schema parse due to reasoning/truncation; local schema validation is mandatory.
  - Increasing completion budget can turn a truncated helper response into a parseable helper result, but retry must be bounded and observable.
  - Token Plan budget predictability does not eliminate weekly quota, peak-hour, RPM/TPM, or production-use caveats.
  - The remains endpoint requires a properly authorized Token Plan Key; a standard/API key may still call chat but return 403 on remains.
---

# M014-65dlgp: MiniMax Real Test and Token Plan Limits

**M014 proved bounded real MiniMax helper callability and documented Token Plan limits while preserving all production safety blocks.**

## What Happened

M014 advanced MiniMax from synthetic smoke-test to real bounded helper testing. S01 documented Token Plan usage visibility through Billing > Token Plan and `/v1/token_plan/remains`, captured subscription budget as non-blocking per user instruction, and preserved platform limits including quotas, RPM/TPM, daily resets, dynamic peak-hour guidance, and weekly quota. The remains endpoint was probed safely and returned HTTP 403 with the current key, so exact live remains remain unknown. S02 ran four real MiniMax chat completion calls over synthetic/redacted metadata only: all returned HTTP 200; strict JSON parsed; an initial redacted helper call truncated; a high-budget retry parsed; and a deliberate low-token edge recorded failure behavior. S03 independent review initially flagged missing weekly/peak-hour details, then passed after correction. R042 was validated. MiniMax is now eligible only for a dev helper adapter probe with local schema validation and bounded retry; production/source-of-truth/orchestration/import/write paths remain blocked.

## Success Criteria Results

- Token Plan limits documented: met.
- Real MiniMax live tests: met, 4 HTTP 200 calls.
- Budget non-blocking but limits respected: met.
- Evidence hygiene: met.
- Independent review: PASS.
- Production blocks: preserved.

## Definition of Done Results

- Token Plan usage visibility documented: met.
- Budget posture captured: subscription budget non-blocking, platform limits still apply.
- Real MiniMax tests run: met with 4 HTTP 200 calls.
- Evidence hygiene: met, no raw responses/model content/secrets/raw paper/chunk text persisted.
- Independent review: PASS after adding weekly quota/peak-hour details.
- R042: validated.
- Production/source-of-truth/orchestration blocks: preserved.

## Requirement Outcomes

- R042 validated with Token Plan and real-call evidence.
- R040 advanced via infrastructure-before-activation discipline.
- No requirement authorizes production import, source-of-truth use, or LadybugDB writes.

## Deviations

Remains endpoint returned HTTP 403 with current key, so exact current quota was not retrieved. This was recorded as an access/credential-type limitation. A fourth live call was run after a truncated helper response to prove high-budget retry behavior within the six-call cap.

## Follow-ups

Next safe work: dev-only MiniMax redacted-metadata helper adapter probe with local JSON schema validation, bounded retry on length/truncation, response-hash-only artifacts, and no fact promotion. Before sustained use, verify active Token Plan tier/purchase timestamp/current remains via Billing > Token Plan or an authorized Token Plan Key.
