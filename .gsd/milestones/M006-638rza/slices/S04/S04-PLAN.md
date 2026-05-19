# S04: Review and recommendation

**Goal:** Independently review the 30-paper deviation scan for semantic usefulness, confirm or challenge the identified patterns, and produce final recommendations for remediation and a future +10-to-100 validation automation CLI milestone.
**Demo:** After this slice, independent review confirms whether the 30-paper scan meaningfully identifies deviations and what next remediation should be.

## Must-Haves

- Independent review checks whether S03 patterns are meaningful and not count-only.
- Final recommendation distinguishes source-readiness, Markdown-based chunking deviations, PDF/multimodal caveats, and import-readiness blockers.
- Future +10-to-100 automation requirements are concrete enough to plan a CLI milestone.
- Positive KG import remains blocked.
- Review artifacts are redacted and do not include raw paper/chunk text.

## Proof Level

- This slice proves: Independent artifact review plus final verification over S03 evidence and S04 recommendation artifacts.

## Integration Closure

Consumes S03 run evidence/report and closes M006 with reviewed recommendations for the next milestone. The review must preserve no-import/no-write boundaries and distinguish observed patterns from unproven KG readiness.

## Verification

- Adds independent review summary, final recommendation report, and explicit go/block criteria for future automation work.

## Tasks

- [x] **T01: Review thirty paper deviation evidence** `est:medium`
  Run an independent review of S03 run evidence and report. The reviewer should assess whether patterns are semantically meaningful, whether outlier flags are useful, whether claims are over-stated, and whether the evidence supports planning an iterative +10-to-100 automation CLI.
  - Files: `.gsd/milestones/M006-638rza/slices/S04/run-evidence/thirty-paper-deviation-review-summary.md`
  - Verify: test -s .gsd/milestones/M006-638rza/slices/S04/run-evidence/thirty-paper-deviation-review-summary.md && grep -q 'Verdict' .gsd/milestones/M006-638rza/slices/S04/run-evidence/thirty-paper-deviation-review-summary.md

- [x] **T02: Write final recommendation for automation milestone** `est:medium`
  Write the final M006 recommendation report. Include what changed from 10 to 30 papers, which patterns matter, what remains blocked, and what an M007 CLI automation milestone should implement.
  - Files: `.gsd/milestones/M006-638rza/slices/S04/thirty-paper-final-recommendation.md`
  - Verify: test -s .gsd/milestones/M006-638rza/slices/S04/thirty-paper-final-recommendation.md && grep -q 'M007' .gsd/milestones/M006-638rza/slices/S04/thirty-paper-final-recommendation.md && grep -q 'positive KG import remains blocked' .gsd/milestones/M006-638rza/slices/S04/thirty-paper-final-recommendation.md

## Files Likely Touched

- .gsd/milestones/M006-638rza/slices/S04/run-evidence/thirty-paper-deviation-review-summary.md
- .gsd/milestones/M006-638rza/slices/S04/thirty-paper-final-recommendation.md
