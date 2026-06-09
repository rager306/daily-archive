# S03: S03

**Goal:** Implement structured-output helper boundary using forced tool-call schema validation over synthetic/redacted data.
**Demo:** After S03, structured MiniMax helper output has a safe local wrapper contract.

## Must-Haves

- Anthropic-compatible forced tool-call contract documented in code/tests.
- Local schema validation required.
- Prompt-only JSON not accepted as proof.
- Raw corpus content remains blocked.

## Proof Level

- This slice proves: Fixture tests and local validation failures.

## Integration Closure

Provides a bounded future review/repair helper, not KG authority.

## Verification

- Adds schema validation diagnostics and failure states.

## Tasks

- [x] **T01: Implemented and tested the MiniMax structured helper boundary with forced tool calls and local schema validation.** `est:medium`
  Implement a dev-only MiniMax structured helper boundary with pure request-building and response-validation primitives. Cover forced Anthropic tool-call payload shape, local input_schema validation, prompt-only JSON rejection, raw corpus payload blocking, non-authoritative outputs, and sanitized diagnostics with tests. Use global skill `minimax-safe-helper` as reference.
  - Files: `src/arxiv_archive/minimax_structured.py`, `tests/test_minimax_structured.py`, `.gsd/milestones/M017-cf3fd0/slices/S03/run-evidence/minimax-structured-helper-guard.json`
  - Verify: uv run pytest tests/test_minimax_structured.py -q && uv run ruff check src/arxiv_archive/minimax_structured.py tests/test_minimax_structured.py && uv run python - <<'PY'
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

## Files Likely Touched

- src/arxiv_archive/minimax_structured.py
- tests/test_minimax_structured.py
- .gsd/milestones/M017-cf3fd0/slices/S03/run-evidence/minimax-structured-helper-guard.json
