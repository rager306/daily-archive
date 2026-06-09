# S03: S03

**Goal:** Advance MiniMax from no-call dry run to an explicit bounded smoke-test decision, and run only if safety/approval constraints are satisfied.
**Demo:** A MiniMax synthetic smoke-test artifact records whether a live synthetic call was run or intentionally deferred, with exact auth/header/schema findings if run.

## Must-Haves

- No raw paper/chunk text is sent or stored.
- Secrets are never logged.
- If live call is run, it uses synthetic prompt only and validates JSON locally.
- If not run, artifact states exact blocker/precondition.
- MiniMax remains helper-only, not orchestrator/source of truth.

## Proof Level

- This slice proves: Bounded synthetic probe or explicit blocker artifact.

## Integration Closure

Provides MiniMax callability evidence or a precise blocker for final recommendation.

## Verification

- Records endpoint/header choice, payload hash, response/schema status or deferral reason, with no secrets/raw text.

## Tasks

- [x] **T01: MiniMax synthetic smoke test succeeded with HTTP 200 using synthetic-only input.** `est:medium`
  Determine whether explicit approval and environment allow a MiniMax synthetic live smoke test. If approval is absent, record deferral; if present, run one tiny synthetic call without raw paper/chunk text.
  - Files: `.gsd/milestones/M013-tdtle0/slices/S03/run-evidence/minimax-smoke-test.json`
  - Verify: test -s .gsd/milestones/M013-tdtle0/slices/S03/run-evidence/minimax-smoke-test.json

- [x] **T02: Wrote MiniMax smoke-test guard: callability proven for synthetic prompt; orchestration/import remain blocked.** `est:small`
  Write MiniMax smoke-test guard with callability status, schema validation status, and blocked behaviors.
  - Files: `.gsd/milestones/M013-tdtle0/slices/S03/run-evidence/minimax-smoke-test-guard.json`
  - Verify: test -s .gsd/milestones/M013-tdtle0/slices/S03/run-evidence/minimax-smoke-test-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M013-tdtle0/slices/S03/run-evidence/minimax-smoke-test-guard.json').read_text())
assert g['secrets_logged'] is False
assert g['minimax_orchestrator_allowed'] is False
print('minimax-smoke-test-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M013-tdtle0/slices/S03/run-evidence/minimax-smoke-test.json
- .gsd/milestones/M013-tdtle0/slices/S03/run-evidence/minimax-smoke-test-guard.json
