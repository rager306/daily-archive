---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Run or defer MiniMax synthetic smoke test

Determine whether explicit approval and environment allow a MiniMax synthetic live smoke test. If approval is absent, record deferral; if present, run one tiny synthetic call without raw paper/chunk text.

## Inputs

- `.gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-compatibility-guard.json`

## Expected Output

- `.gsd/milestones/M013-tdtle0/slices/S03/run-evidence/minimax-smoke-test.json`

## Verification

test -s .gsd/milestones/M013-tdtle0/slices/S03/run-evidence/minimax-smoke-test.json

## Observability Impact

Records live/deferred status and safe payload hash.
