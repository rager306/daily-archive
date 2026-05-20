# S02: MiniMax compatibility spike

**Goal:** Research and, if credentials are available via secure collection, run a bounded MiniMax callability probe without making MiniMax an orchestrator or source of truth.
**Demo:** A MiniMax compatibility report exists with API/auth/model/modalities, structured output feasibility, adapter options, and bounded call policy.

## Must-Haves

- Current MiniMax API/auth/model/modalities are documented from https://platform.minimax.io/docs/api-reference/api-overview and linked docs.
- Structured output and image/PDF repair feasibility are assessed.
- If live call is run, it uses secure_env_collect and redacted non-production input.
- Marker/custom adapter implications are documented.
- MiniMax remains optional helper only, not orchestrator/source of truth.

## Proof Level

- This slice proves: Current docs plus optional bounded live probe evidence.

## Integration Closure

Produces MiniMax compatibility artifact for the combined integration matrix.

## Verification

- Records docs consulted, auth requirements, call/probe status, redaction policy, rate/cost risks, and adapter implications.

## Tasks

- [x] **T01: Research MiniMax official API requirements** `est:medium`
  Use official MiniMax docs starting at https://platform.minimax.io/docs/api-reference/api-overview to document auth, base URL, model families, text/image/audio/video capabilities, structured output/tool support if available, rate/cost considerations, and SDK/API invocation shape.
  - Files: `.gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-research-report.md`
  - Verify: test -s .gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-research-report.md

- [x] **T02: Assess MiniMax bounded callability probe** `est:medium`
  Determine whether a bounded live MiniMax call can be run safely. If a required key is missing, use secure_env_collect before any live probe; otherwise record skipped status. Probe must use redacted non-production input and never include raw paper/chunk text.
  - Files: `.gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-probe.json`
  - Verify: test -s .gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-probe.json

- [x] **T03: Write MiniMax compatibility guard** `est:small`
  Synthesize MiniMax findings into a compatibility guard with go/no-go, preconditions, adapter implications, and blocked orchestrator/source-of-truth behavior.
  - Files: `.gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-compatibility-guard.json`, `.gsd/milestones/M012-a7v8fw/slices/S02/minimax-compatibility-summary.md`
  - Verify: test -s .gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-compatibility-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-compatibility-guard.json').read_text())
assert g['production_import_attempted'] is False
assert g['minimax_orchestrator_allowed'] is False
print('minimax-compatibility-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-research-report.md
- .gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-probe.json
- .gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-compatibility-guard.json
- .gsd/milestones/M012-a7v8fw/slices/S02/minimax-compatibility-summary.md
