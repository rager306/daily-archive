---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Ran four real MiniMax helper probes: HTTP 200 for all, strict JSON and redacted helper succeeded with schema-validation caveats.

Run bounded live MiniMax helper probes: strict JSON, redacted KG helper decision, and deliberate schema/length edge. Persist sanitized metadata only.

## Inputs

- `.gsd/milestones/M014-65dlgp/slices/S01/run-evidence/token-plan-limits-guard.json`

## Expected Output

- `.gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-helper-probes.json`

## Verification

uv run python - <<'PY'
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

## Observability Impact

Captures real MiniMax call statuses, schema outcomes, hashes, latency, usage metadata, and redaction flags.
