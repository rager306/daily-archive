---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T01: Implement MiniMax structured helper boundary

Implement a dev-only MiniMax structured helper boundary with pure request-building and response-validation primitives. Cover forced Anthropic tool-call payload shape, local input_schema validation, prompt-only JSON rejection, raw corpus payload blocking, non-authoritative outputs, and sanitized diagnostics with tests. Use global skill `minimax-safe-helper` as reference.

## Inputs

- None specified.

## Expected Output

- `src/arxiv_archive/minimax_structured.py`
- `tests/test_minimax_structured.py`
- `.gsd/milestones/M017-cf3fd0/slices/S03/run-evidence/minimax-structured-helper-guard.json`

## Verification

uv run pytest tests/test_minimax_structured.py -q && uv run ruff check src/arxiv_archive/minimax_structured.py tests/test_minimax_structured.py && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M017-cf3fd0/slices/S03/run-evidence/minimax-structured-helper-guard.json').read_text())
assert g['forced_tool_call_required'] is True
assert g['local_schema_validation_required'] is True
assert g['prompt_only_json_accepted'] is False
assert g['raw_corpus_payload_allowed'] is False
assert g['minimax_source_of_truth'] is False
print('minimax-structured-helper-guard-ok')
PY

## Observability Impact

Structured helper guard records schema validation and raw payload boundaries.
