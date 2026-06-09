---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Built integration matrix: both DSPy and MiniMax are future bounded-probe candidates only, not production activations.

Build a combined compatibility matrix comparing DSPy and MiniMax roles, current status, next safe probes, blocked behaviors, and activation preconditions.

## Inputs

- `.gsd/milestones/M012-a7v8fw/slices/S01/run-evidence/dspy-compatibility-guard.json`
- `.gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-compatibility-guard.json`

## Expected Output

- `.gsd/milestones/M012-a7v8fw/slices/S03/integration-boundary-matrix.md`
- `.gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-matrix.json`

## Verification

test -s .gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-matrix.json

## Observability Impact

Matrix becomes the cross-track synthesis for S04 review.
