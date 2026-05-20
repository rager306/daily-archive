# S03: Integration boundary matrix

**Goal:** Map DSPy and MiniMax findings onto the current pipeline boundaries, artifact contracts, and post-M011 chunk-span/candidate-locator needs.
**Demo:** A combined integration matrix shows where DSPy and MiniMax could fit, what each requires, and which gates must pass before activation.

## Must-Haves

- Matrix distinguishes ready, blocked, optional, and future-only surfaces.
- No-import/no-write/secret/redaction constraints are represented.
- Chunk-span provenance and candidate-locator dependency is explicitly connected.
- DSPy and MiniMax activation preconditions are separate, not conflated.

## Proof Level

- This slice proves: Cross-artifact consistency review.

## Integration Closure

Consumes S01 and S02 research and provides synthesis input for final recommendation.

## Verification

- Adds compatibility matrix, failure mode table, and activation preconditions.

## Tasks

- [x] **T01: Build combined compatibility matrix** `est:medium`
  Build a combined compatibility matrix comparing DSPy and MiniMax roles, current status, next safe probes, blocked behaviors, and activation preconditions.
  - Files: `.gsd/milestones/M012-a7v8fw/slices/S03/integration-boundary-matrix.md`, `.gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-matrix.json`
  - Verify: test -s .gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-matrix.json

- [x] **T02: Write integration guard** `est:small`
  Write a failure-mode and activation-precondition guard that proves DSPy and MiniMax remain disabled in the production process and identifies exact next probes.
  - Files: `.gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-guard.json`
  - Verify: test -s .gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-guard.json').read_text())
assert g['dspy_production_runtime_allowed'] is False
assert g['minimax_orchestrator_allowed'] is False
assert g['production_import_allowed'] is False
print('integration-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M012-a7v8fw/slices/S03/integration-boundary-matrix.md
- .gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-matrix.json
- .gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-guard.json
