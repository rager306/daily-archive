---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Defined a conservative semantic import-readiness rubric that blocks import candidates without chunk-level span provenance.

Write a semantic import-readiness rubric that can classify targets as import_candidate, retrieval_only, repair_required, or reject, with explicit blockers for missing chunk spans and no trusted claim text.

## Inputs

- `.gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/semantic-review-targets.json`

## Expected Output

- `.gsd/milestones/M011-2f8j8m/slices/S02/semantic-review-rubric.md`

## Verification

test -s .gsd/milestones/M011-2f8j8m/slices/S02/semantic-review-rubric.md

## Observability Impact

Rubric gives a durable decision surface for future semantic gates.
