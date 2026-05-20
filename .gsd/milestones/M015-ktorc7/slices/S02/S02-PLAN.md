# S02: MiniMax structured JSON remediation

**Goal:** Debug MiniMax structured output using Anthropic-compatible recommended API, tool calls, and OpenAI-compatible structured options.
**Demo:** After S02, we know whether MiniMax supports structured output reliably via prompt JSON, reasoning split, response_format, or tool calls.

## Must-Haves

- Anthropic text API is tested.
- Anthropic forced tool_call/input_schema is tested.
- OpenAI reasoning_split is tested.
- OpenAI response_format/json_object/json_schema support is tested.
- Artifacts persist no raw prompts/responses/model content.
- Final guard names recommended structured interface or explicit blocker.

## Proof Level

- This slice proves: Live MiniMax API matrix with local schema validation.

## Integration Closure

Provides corrected structured-output verdict to S03.

## Verification

- Sanitized matrix of structured-output success/failure and interface recommendation.

## Tasks

- [x] **T01: Run structured-output API matrix** `est:medium`
  Run live MiniMax structured-output matrix with sanitized artifacts: Anthropic text, Anthropic forced tool, OpenAI reasoning_split, OpenAI response_format variants.
  - Files: `.gsd/milestones/M015-ktorc7/slices/S02/run-evidence/minimax-structured-output-matrix.json`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
d=json.loads(Path('.gsd/milestones/M015-ktorc7/slices/S02/run-evidence/minimax-structured-output-matrix.json').read_text())
assert d['live_call_count'] >= 4
assert d['raw_response_persisted'] is False
assert d['raw_model_content_persisted'] is False
assert d['secrets_logged'] is False
print('minimax-structured-output-matrix-ok')
PY

- [x] **T02: Write structured-output verdict** `est:small`
  Write structured-output verdict naming which MiniMax interface should be used for helper adapter and what controls are required.
  - Files: `.gsd/milestones/M015-ktorc7/slices/S02/run-evidence/minimax-structured-output-guard.json`, `.gsd/milestones/M015-ktorc7/slices/S02/minimax-structured-output-remediation.md`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M015-ktorc7/slices/S02/run-evidence/minimax-structured-output-guard.json').read_text())
assert g['raw_response_persisted'] is False
assert g['raw_model_content_persisted'] is False
assert g['production_import_allowed'] is False
assert g['structured_output_verdict'] in {'tool_call_recommended','prompt_json_with_controls','blocked','mixed'}
print('minimax-structured-output-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M015-ktorc7/slices/S02/run-evidence/minimax-structured-output-matrix.json
- .gsd/milestones/M015-ktorc7/slices/S02/run-evidence/minimax-structured-output-guard.json
- .gsd/milestones/M015-ktorc7/slices/S02/minimax-structured-output-remediation.md
