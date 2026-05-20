---
id: S02
parent: M013-tdtle0
milestone: M013-tdtle0
provides:
  - DSPy optimizer applicability catalog
  - optimizer guard
requires:
  []
affects:
  - S04
key_files:
  - .gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-guard.json
  - .gsd/milestones/M013-tdtle0/slices/S02/dspy-optimizer-applicability-catalog.md
key_decisions:
  - KNNFewShot and LabeledFewShot are the only possible-dev first candidates, and only after span-labeled devset exists.
  - MIPROv2, COPRO, SIMBA, and BootstrapFewShot variants are future-only.
  - GEPA, BetterTogether, and BootstrapFinetune are blocked for now.
patterns_established:
  - Start DSPy optimizer exploration with low-fanout demo selection, not broad prompt optimizers.
  - Optimizer applicability depends on chunk-span provenance, labeled devsets, metrics, budget caps, and trace redaction.
observability_surfaces:
  - optimizer inventory
  - applicability catalog
  - optimizer guard
drill_down_paths:
  - .gsd/milestones/M013-tdtle0/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M013-tdtle0/slices/S02/tasks/T02-SUMMARY.md
  - .gsd/milestones/M013-tdtle0/slices/S02/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-20T10:47:00.153Z
blocker_discovered: false
---

# S02: DSPy optimizer applicability catalog

**S02 produced the DSPy optimizer map: KNN/Labeled few-shot possible later; advanced optimizers future-only or blocked.**

## What Happened

S02 inventoried 19 DSPy optimizer/support classes and classified applicability for Scientific KG extraction. Results: possible-dev=2 (KNNFewShot, LabeledFewShot), future-only=6 (BootstrapFewShot variants, MIPROv2, COPRO, SIMBA), blocked=3 (GEPA, BetterTogether, BootstrapFinetune), not-applicable-now=8. The guard confirms no optimizer was run and production import remains blocked.

## Verification

Fresh combined check passed: possible_dev_optimizers=['KNNFewShot','LabeledFewShot'], optimizer_executed=false.

## Requirements Advanced

- R041 — S02 satisfies the optimizer-details portion of R041.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

None.

## Known Limitations

Catalog is based on static source inspection and project constraints; no optimizer was executed.

## Follow-ups

S04 should recommend KNNFewShot or LabeledFewShot only as possible-dev future candidates after span-labeled devset and metrics exist. MIPROv2/GEPA/etc. should remain later or blocked.

## Files Created/Modified

- `.gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-inventory.json` — Optimizer inventory.
- `.gsd/milestones/M013-tdtle0/slices/S02/dspy-optimizer-applicability-catalog.md` — Optimizer applicability catalog.
- `.gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-applicability.json` — Machine-readable applicability ratings.
- `.gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-guard.json` — Optimizer guard.
