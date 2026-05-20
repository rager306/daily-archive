---
id: T02
parent: S02
milestone: M013-tdtle0
key_files:
  - .gsd/milestones/M013-tdtle0/slices/S02/dspy-optimizer-applicability-catalog.md
  - .gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-applicability.json
key_decisions:
  - Classify KNNFewShot and LabeledFewShot as possible-dev first candidates after span-labeled devset exists.
  - Classify MIPROv2, COPRO, SIMBA, Bootstrap variants as future-only until metrics/devsets/budget/redaction guards exist.
  - Classify GEPA, BetterTogether, and BootstrapFinetune as blocked for now due to trace/fanout/finetuning risk.
duration: 
verification_result: passed
completed_at: 2026-05-20T10:45:20.527Z
blocker_discovered: false
---

# T02: Assessed DSPy optimizers: 2 possible-dev, 6 future-only, 3 blocked, 8 not applicable now.

**Assessed DSPy optimizers: 2 possible-dev, 6 future-only, 3 blocked, 8 not applicable now.**

## What Happened

Assessed DSPy optimizer applicability for Scientific KG extraction. Ratings: possible-dev=2 (KNNFewShot, LabeledFewShot), future-only=6 (BootstrapFewShot variants, MIPROv2, COPRO, SIMBA), blocked=3 (GEPA, BetterTogether, BootstrapFinetune), not-applicable-now=8 support/internal classes. Required gates before any optimizer: chunk-span provenance, candidate fact locators, redacted labeled devset, ExtractionPatch metrics, budget caps, trace redaction policy, and explicit go decision.

## Verification

dspy-optimizer-applicability.json exists and records optimizer_count=19, optimizer_executed=false, and rating counts.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `write dspy-optimizer-applicability.json and catalog markdown` | 0 | ✅ pass — possible-dev=2; future-only=6; blocked=3; optimizer_executed=false | 11000ms |

## Deviations

None.

## Known Issues

No optimizer is applicable to production now. Possible-dev means only after chunk-span/candidate-locator evidence and redacted devset/metrics exist.

## Files Created/Modified

- `.gsd/milestones/M013-tdtle0/slices/S02/dspy-optimizer-applicability-catalog.md`
- `.gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-applicability.json`
