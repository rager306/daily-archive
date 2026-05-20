---
id: S01
parent: M008-c9zb94
milestone: M008-c9zb94
provides:
  - M008 new +10 corpus manifest
  - Selection rationale
  - Overlap and source availability preview
requires:
  []
affects:
  - S02
  - S03
  - S04
key_files:
  - .gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json
  - .gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-availability-report.md
key_decisions:
  - Use first 10 lexicographically sorted local/cache candidate IDs after excluding M006 30 papers.
  - Do not select only easy Markdown-ready papers; this is the first real workflow stress test.
  - Keep S01 redacted and metadata-only.
patterns_established:
  - New +10 selection uses deterministic lexicographic inventory after excluding prior corpus.
  - Selection should not be biased only toward source-ready papers when the goal is testing workflow robustness.
observability_surfaces:
  - candidate inventory with source availability counts
  - new +10 manifest with per-paper path/status metadata
  - availability report with overlap and source preview
drill_down_paths:
  - .gsd/milestones/M008-c9zb94/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M008-c9zb94/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M008-c9zb94/slices/S01/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-20T02:22:56.697Z
blocker_discovered: false
---

# S01: Select first new plus ten corpus

**S01 selected a deterministic new +10 batch with no M006 overlap and redacted source preview.**

## What Happened

S01 selected the first genuinely new +10-paper corpus. It built a redacted inventory of 800 non-M006 candidates from local research/cache metadata, selected the first 10 lexicographically sorted candidates, verified no overlap with the prior M006 30-paper corpus, and documented source availability. The selected batch is intentionally real rather than easy: only 1/10 papers has Markdown before S02, and 1/10 has a cached PDF. No acquisition, scan, KG import, or production writes occurred.

## Verification

Fresh slice verification passed: candidate_count=800, selected_count=10, overlap_count=0, markdown_available=1, pdf_available=1, raw_text_included=false.

## Requirements Advanced

- R034 — S01 selects the first genuinely new +10 corpus required by R034.
- R032 — S01 begins exercising the iterative +10 workflow beyond the M006/M007 existing 30-paper proof.

## Requirements Validated

None.

## New Requirements Surfaced

- S02 may need bounded acquisition/repair helper improvements if most selected papers lack Markdown.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

None.

## Known Limitations

Only 1/10 selected papers has existing Markdown and 1/10 has cached PDF. S02 may discover source acquisition blockers.

## Follow-ups

S02 should run validation-batch init/preflight and then bounded acquisition/repair because only 1/10 selected papers currently has Markdown.

## Files Created/Modified

- `.gsd/milestones/M008-c9zb94/slices/S01/run-evidence/new-plus-ten-candidate-inventory.json` — Candidate inventory excluding M006 papers.
- `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json` — Deterministic selected +10 manifest.
- `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-selection-rationale.md` — Selection rationale.
- `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-availability-report.md` — Availability and overlap report.
