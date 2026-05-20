---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Review quota-filled new plus ten evidence

Run independent review of M008 S01-S03 artifacts. Focus on whether quota-fill evidence is meaningful, source readiness is honestly represented, scan artifacts are redacted, and claims do not exceed evidence.

## Inputs

- `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json`
- `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-preflight-summary.json`
- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-summary.json`
- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/validation-scan-summary.json`
- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/delta-report.json`
- `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/outlier-report.json`

## Expected Output

- `.gsd/milestones/M008-c9zb94/slices/S04/run-evidence/new-plus-ten-review-summary.md`

## Verification

test -s .gsd/milestones/M008-c9zb94/slices/S04/run-evidence/new-plus-ten-review-summary.md && grep -Fq 'Verdict:' .gsd/milestones/M008-c9zb94/slices/S04/run-evidence/new-plus-ten-review-summary.md

## Observability Impact

Independent review compresses artifact meaning and flags evidence gaps before milestone close.
