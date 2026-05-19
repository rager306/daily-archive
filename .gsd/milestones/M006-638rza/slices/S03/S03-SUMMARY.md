---
id: S03
parent: M006-638rza
milestone: M006-638rza
provides:
  - 30-paper structure-aware deviation evidence
  - M005 baseline comparison
  - Outlier list and route-share pattern taxonomy
  - Inputs for S04 independent review and future +10 automation planning
requires:
  - slice: S01
    provides: 30-paper corpus manifest and availability audit.
  - slice: S02
    provides: 30/30 Markdown-ready corpus and source caveats.
affects:
  - S04
  - future iterative validation CLI milestone
key_files:
  - src/arxiv_archive/thirty_paper_deviation_scan.py
  - .gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-summary.json
  - .gsd/milestones/M006-638rza/slices/S03/thirty-paper-deviation-report.md
key_decisions:
  - Compare route shares primarily against M005/S03 structure-aware evidence for apples-to-apples analysis.
  - Use M005/S06 only as mixed benchmark/import-boundary context.
  - Keep positive KG import blocked because import eligibility remains zero across 4,289 chunks.
patterns_established:
  - 30-paper scan reveals route-share shifts hidden by the 10-paper baseline.
  - Method/table/figure/citation routes need first-class review metrics in future automation.
  - Outlier detection should become a standard part of each +10 batch.
  - Markdown readiness does not imply PDF/multimodal completeness.
observability_surfaces:
  - .gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-summary.json — aggregate distribution and baseline comparison
  - .gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-diagnostics.jsonl — per-paper redacted metrics
  - .gsd/milestones/M006-638rza/slices/S03/thirty-paper-deviation-report.md — human-readable pattern taxonomy and recommendations
drill_down_paths:
  - .gsd/milestones/M006-638rza/slices/S03/tasks/T01-SUMMARY.md
  - .gsd/milestones/M006-638rza/slices/S03/tasks/T02-SUMMARY.md
  - .gsd/milestones/M006-638rza/slices/S03/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-19T18:09:25.671Z
blocker_discovered: false
---

# S03: Deviation and pattern analysis

**S03 found meaningful 30-paper route shifts and 11 outliers while confirming zero import eligibility.**

## What Happened

S03 ran the first full 30-paper Markdown-based deviation scan. It produced 4,289 structure-aware chunks over 1,761,102 Markdown bytes, 17,573 annotations, and 30 per-paper diagnostics. Import eligibility remains zero and all chunks remain refused for trusted KG import. Compared with M005/S03, retrieval-only remains dominant but drops from 76.41% to 70.09%, while method, figure, citation, claim, and table routes become more visible. The scan flagged 11 outlier papers, including high-chunk-count and claim/table-heavy cases. These findings support the user's proposed +10 iterative loop, but only with source preflight, bounded acquisition, route-share deltas, and review gates.

## Verification

Fresh slice verification passed: 34 focused tests passed, ruff passed, and artifact guard confirmed 30 papers, 4,289 chunks, 11 outliers, zero import eligibility, and all no-import/no-write/no-payload safety flags false.

## Requirements Advanced

- R031 — S03 delivers the first 30-paper deviation scan, with concrete new patterns and outliers compared against M005.
- R030 — S03 reinforces that source/PDF completeness must stay separate from Markdown-based chunking analysis.
- R029 — S03 confirms that broader structure-aware chunks still require review and remain import-ineligible.
- R032 — S03 provides concrete route-share/outlier requirements for future +10 batch automation.

## Requirements Validated

None.

## New Requirements Surfaced

- Future automation should include per-batch route-share deltas, outlier flags, source readiness preflight, and bounded targeted repair before scan analysis.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

The scanner initially wrote `markdown_byte_size_total=0`; this was corrected before task completion by computing byte size from the selected full_text.md path and regenerating evidence. S03 remains Markdown-based, not PDF/multimodal complete.

## Known Limitations

S03 is Markdown-based. Cached PDFs are available for only 8/30 papers, so multimodal/PDF completeness remains unproven. Outlier flags are deterministic heuristics and need independent review.

## Follow-ups

S04 should independently review whether the 30-paper patterns are semantically meaningful and sufficient to plan the +10-to-100 automation CLI milestone.

## Files Created/Modified

- `src/arxiv_archive/thirty_paper_deviation_scan.py` — Thirty-paper deviation scanner and writer.
- `tests/test_thirty_paper_deviation_scan.py` — Tests for redaction, baseline comparison, and artifact writing.
- `.gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-summary.json` — Aggregate 30-paper deviation summary.
- `.gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-diagnostics.jsonl` — Per-paper redacted deviation diagnostics.
- `.gsd/milestones/M006-638rza/slices/S03/thirty-paper-deviation-report.md` — Human-readable M005 baseline comparison and pattern report.
