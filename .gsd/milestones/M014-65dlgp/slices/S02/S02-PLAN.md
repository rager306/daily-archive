# S02: S02

**Goal:** Run real MiniMax live API helper probes using bounded synthetic/redacted inputs and sanitized artifacts.
**Demo:** After S02, MiniMax has been exercised with real bounded helper-style API calls and sanitized evidence.

## Must-Haves

- Real MiniMax API calls run using bounded inputs.
- Strict JSON and redacted helper-style behavior are evaluated.
- At least one edge/failure behavior is recorded.
- No raw response/model content, secrets, raw paper text, chunk text, embeddings, or vectors are persisted.
- MiniMax remains helper-only and non-authoritative.

## Proof Level

- This slice proves: Live API evidence plus local JSON guards.

## Integration Closure

S02 produces callability/schema evidence for S03 recommendation.

## Verification

- Adds per-call sanitized metadata, aggregate guard, and schema/failure behavior.

## Tasks

- [x] **T01: Ran four real MiniMax helper probes: HTTP 200 for all, strict JSON and redacted helper succeeded with schema-validation caveats.** `est:medium`
  Run bounded live MiniMax helper probes: strict JSON, redacted KG helper decision, and deliberate schema/length edge. Persist sanitized metadata only.
  - Files: `.gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-helper-probes.json`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
p=Path('.gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-helper-probes.json')
d=json.loads(p.read_text())
assert d['live_call_count'] >= 3
assert d['raw_response_persisted'] is False
assert d['raw_model_content_persisted'] is False
assert d['secrets_logged'] is False
assert d['raw_project_text_included'] is False
print('minimax-real-helper-probes-ok')
PY

- [x] **T02: Wrote MiniMax real-test guard: real helper probe can continue only with schema validation and retry controls.** `est:small`
  Write a real-test guard that summarizes pass/flag outcomes, schema reliability, redaction hygiene, and blocked scopes.
  - Files: `.gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-test-guard.json`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-test-guard.json').read_text())
assert g['live_call_count'] >= 3
assert g['raw_response_persisted'] is False
assert g['raw_model_content_persisted'] is False
assert g['minimax_orchestrator_allowed'] is False
assert g['production_import_allowed'] is False
print('minimax-real-test-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-helper-probes.json
- .gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-test-guard.json
