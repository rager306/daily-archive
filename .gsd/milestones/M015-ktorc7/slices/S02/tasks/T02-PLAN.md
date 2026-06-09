---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Wrote corrected MiniMax structured-output verdict: use Anthropic forced tool calls with schema validation.

Write structured-output verdict naming which MiniMax interface should be used for helper adapter and what controls are required.

## Inputs

- `.gsd/milestones/M015-ktorc7/slices/S02/run-evidence/minimax-structured-output-matrix.json`

## Expected Output

- `.gsd/milestones/M015-ktorc7/slices/S02/run-evidence/minimax-structured-output-guard.json`
- `.gsd/milestones/M015-ktorc7/slices/S02/minimax-structured-output-remediation.md`

## Verification

uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M015-ktorc7/slices/S02/run-evidence/minimax-structured-output-guard.json').read_text())
assert g['raw_response_persisted'] is False
assert g['raw_model_content_persisted'] is False
assert g['production_import_allowed'] is False
assert g['structured_output_verdict'] in {'tool_call_recommended','prompt_json_with_controls','blocked','mixed'}
print('minimax-structured-output-guard-ok')
PY

## Observability Impact

Corrected structured-output recommendation.
