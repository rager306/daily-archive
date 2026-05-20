---
id: S03
parent: M012-a7v8fw
milestone: M012-a7v8fw
provides:
  - Integration matrix
  - activation boundary guard
  - next safe options
requires:
  - slice: S01
    provides: DSPy compatibility guard.
  - slice: S02
    provides: MiniMax compatibility guard.
affects:
  - S04
key_files:
  - .gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-guard.json
  - .gsd/milestones/M012-a7v8fw/slices/S03/integration-boundary-matrix.md
key_decisions:
  - DSPy and MiniMax are separate optional future probes, not a combined activation.
  - Production import remains blocked until chunk-span provenance and candidate locators exist.
  - MiniMax live call requires explicit approval even with key present.
patterns_established:
  - Parallel research tracks should synthesize into a matrix before final recommendations.
  - Compatibility findings must keep tool-specific preconditions separate.
observability_surfaces:
  - integration matrix
  - integration boundary markdown
  - integration guard
drill_down_paths:
  - .gsd/milestones/M012-a7v8fw/slices/S03/tasks/T01-SUMMARY.md
  - .gsd/milestones/M012-a7v8fw/slices/S03/tasks/T02-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-20T10:24:41.848Z
blocker_discovered: false
---

# S03: Integration boundary matrix

**S03 synthesized the research: both tools are only future bounded-probe candidates, not production activations.**

## What Happened

S03 synthesized DSPy and MiniMax compatibility findings. The matrix records DSPy as conditionally compatible only for a future optional/dev dependency no-LM probe, with production runtime and optimizers blocked. It records MiniMax as conditionally compatible only for a future optional helper probe, with live call not yet attempted and orchestration/source-of-truth/direct PDF behavior blocked. The integration guard keeps production import and LadybugDB writes blocked and names three next safe options.

## Verification

Fresh S03 check passed: DSPy optional dev probe allowed, MiniMax optional helper probe allowed, production import blocked.

## Requirements Advanced

- R039 — S03 integrates R039's DSPy and MiniMax compatibility evidence into a single go/no-go matrix.
- R040 — S03 enforces R040 by preserving research/probe-before-activation boundaries.

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

S03 is synthesis only; it does not run the DSPy dependency probe or MiniMax live smoke test.

## Follow-ups

S04 should choose final go/no-go/precondition verdicts and update R039. It should avoid recommending broad activation; only bounded next probes or chunk-span packet are justified.

## Files Created/Modified

- `.gsd/milestones/M012-a7v8fw/slices/S03/integration-boundary-matrix.md` — Combined integration matrix.
- `.gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-matrix.json` — Machine-readable integration matrix.
- `.gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-guard.json` — Activation boundary guard.
