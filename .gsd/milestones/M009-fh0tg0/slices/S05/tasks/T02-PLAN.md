---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T02: Write hardening recommendation

Write final recommendation: whether to proceed to the next reviewed +10 batch, and under what required invocation gates.

## Inputs

- `.gsd/milestones/M009-fh0tg0/slices/S05/run-evidence/hardening-review-summary.md`

## Expected Output

- `.gsd/milestones/M009-fh0tg0/slices/S05/hardening-final-recommendation.md`

## Verification

test -s .gsd/milestones/M009-fh0tg0/slices/S05/hardening-final-recommendation.md && grep -Fq 'positive KG import remains blocked' .gsd/milestones/M009-fh0tg0/slices/S05/hardening-final-recommendation.md

## Observability Impact

Recommendation becomes the operational gate for the next milestone.
