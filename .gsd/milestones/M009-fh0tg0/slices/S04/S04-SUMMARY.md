---
id: S04
parent: M009-fh0tg0
milestone: M009-fh0tg0
provides:
  - bounded top-up planner
  - top-up pass evidence
  - top-up shortage/blocker evidence
requires:
  - slice: S03
    provides: Active scan lineage and verifier support.
affects:
  - S05
key_files:
  - src/arxiv_archive/validation_batch_workflow.py
  - tests/test_validation_batch_top_up.py
  - .gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-pass-summary.json
  - .gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-blocked-summary.json
key_decisions:
  - Implement bounded top-up as a read-only planner before acquisition integration.
  - Scan is allowed only when final_accepted_ready_count equals target_count.
  - Unfilled quota writes explicit `bounded_top_up_shortage` blocker diagnostics.
patterns_established:
  - Underfilled batches must produce accepted/rejected replacement diagnostics.
  - Top-up is bounded by max candidate consideration.
  - Planner evidence is separate from actual source acquisition, avoiding hidden unbounded behavior.
observability_surfaces:
  - top-up-pass-summary.json
  - top-up-pass-diagnostics.jsonl
  - top-up-blocked-summary.json
  - top-up-blocked-diagnostics.jsonl
drill_down_paths:
  - .gsd/milestones/M009-fh0tg0/slices/S04/tasks/T01-SUMMARY.md
  - .gsd/milestones/M009-fh0tg0/slices/S04/tasks/T02-SUMMARY.md
  - .gsd/milestones/M009-fh0tg0/slices/S04/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-20T05:23:53.865Z
blocker_discovered: false
---

# S04: Bounded quota top-up automation

**S04 added deterministic bounded top-up planning with both successful replacement and explicit shortage blocker evidence.**

## What Happened

S04 added bounded quota top-up planning. The planner starts from the current quota-fill state, skips already selected candidates, considers replacements in deterministic inventory order up to a maximum bound, accepts only source-ready replacements, records rejected candidates, and computes whether scan is allowed. Sample evidence includes both a successful top-up plan and a blocked shortage plan. The blocked plan proves max bounds are honored and scan remains blocked when quota cannot be filled.

## Verification

Fresh S04 verification passed: pass sample scan_allowed=true/final_ready=3, blocked sample scan_allowed=false/shortage=2, safety flags false, 14 tests passed, and ruff passed.

## Requirements Advanced

- R035 — S04 implements bounded top-up planning and explicit shortage blockers for quota-fill behavior.
- R036 — S04 adds auditable top-up artifacts for validation automation hardening.

## Requirements Validated

None.

## New Requirements Surfaced

- Future integration should connect accepted replacements to actual bounded source acquisition and preflight before scan.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

S04 implements top-up planning/reporting, not real source acquisition or state mutation. This is intentional: bounded acquisition integration remains future execution wiring.

## Known Limitations

Top-up planner does not fetch, convert, or preflight replacement papers. It assumes redacted inventory readiness metadata. A real next +10 flow may still require wiring this planner to bounded source acquisition before scan.

## Follow-ups

S05 should review whether planning/reporting top-up behavior is enough to allow the next +10, or whether actual acquisition-loop integration is still required first.

## Files Created/Modified

- `src/arxiv_archive/validation_batch_workflow.py` — Bounded top-up planner and artifact writer.
- `tests/test_validation_batch_top_up.py` — Top-up behavior tests.
- `.gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-pass-summary.json` — Successful top-up sample summary.
- `.gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-blocked-summary.json` — Blocked shortage sample summary.
