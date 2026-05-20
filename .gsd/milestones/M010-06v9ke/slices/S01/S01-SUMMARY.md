---
id: S01
parent: M010-06v9ke
milestone: M010-06v9ke
provides:
  - M010 next +10 manifest
  - redacted candidate inventory
  - selection and availability guard
requires:
  []
affects:
  - S02
key_files:
  - .gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-corpus-manifest.json
  - .gsd/milestones/M010-06v9ke/slices/S01/run-evidence/selection-guard.json
key_decisions:
  - Exclude all M006 and M008 validation papers.
  - Select deterministic first 10 eligible candidates rather than source-ready-biased papers.
  - Treat 0/10 upfront source readiness as an S02 gate, not an S01 blocker.
patterns_established:
  - Next validation batches should not bias selection toward easy source-ready papers unless explicitly scoped.
  - Source availability gaps are first-class preflight/top-up evidence, not selection failures.
observability_surfaces:
  - next-plus-ten-candidate-inventory.json
  - next-plus-ten-corpus-manifest.json
  - next-plus-ten-availability-report.md
  - selection-guard.json
drill_down_paths:
  - .gsd/milestones/M010-06v9ke/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M010-06v9ke/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M010-06v9ke/slices/S01/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-20T07:13:17.090Z
blocker_discovered: false
---

# S01: Select next gated plus ten corpus

**S01 selected the next +10 corpus: 10 new papers, 0 prior overlap, 0/10 upfront Markdown/PDF.**

## What Happened

S01 selected the next deterministic +10 corpus under the M009-gated milestone. It excluded 40 prior validation IDs from M006 and M008, built a redacted candidate inventory with 790 eligible candidates, and selected the first 10 lexicographically sorted eligible IDs. The selected IDs have no prior overlap and no upfront Markdown/PDF availability, which intentionally exercises S02's bounded acquisition/top-up gates. All artifacts are redacted and no import/write behavior occurred.

## Verification

Fresh S01 verification passed: candidate_count=790, selected_count=10, prior_overlap_count=0, markdown_available_count=0, pdf_available_count=0, safety flags false.

## Requirements Advanced

- R037 — S01 selects a new +10 excluding prior validation corpora.

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

Selected corpus has 0/10 Markdown available and 0/10 PDF available upfront, though 10/10 have research workspace and paper_json metadata. This will require bounded acquisition/materialization/top-up before scan.

## Follow-ups

S02 must use bounded acquisition/top-up gates because the selected batch has 0/10 upfront Markdown/PDF availability.

## Files Created/Modified

- `.gsd/milestones/M010-06v9ke/slices/S01/run-evidence/next-plus-ten-candidate-inventory.json` — Redacted candidate inventory excluding M006/M008 corpora.
- `.gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-corpus-manifest.json` — Selected M010 next +10 manifest.
- `.gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-selection-rationale.md` — Selection rationale.
- `.gsd/milestones/M010-06v9ke/slices/S01/next-plus-ten-availability-report.md` — Availability report.
- `.gsd/milestones/M010-06v9ke/slices/S01/run-evidence/selection-guard.json` — Machine-readable selection guard.
