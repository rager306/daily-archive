---
estimated_steps: 1
estimated_files: 4
skills_used: []
---

# T01: Review and close MiniMax helper safety

Run fresh verification across MiniMax usage and structured helpers, perform an independent safety review, write final guard and recommendation, and update R045. Verify no production KG import, LadybugDB write, raw response, exact quota, secret, raw corpus, embedding/vector, or source-of-truth path is enabled.

## Inputs

- `src/arxiv_archive/minimax_usage.py`
- `src/arxiv_archive/minimax_structured.py`
- `tests/test_minimax_usage.py`
- `tests/test_minimax_structured.py`

## Expected Output

- `.gsd/milestones/M017-cf3fd0/slices/S04/run-evidence/final-m017-guard.json`
- `.gsd/milestones/M017-cf3fd0/slices/S04/run-evidence/m017-independent-review.md`
- `.gsd/milestones/M017-cf3fd0/slices/S04/m017-final-recommendation.md`

## Verification

uv run pytest tests/test_minimax_usage.py tests/test_minimax_structured.py -q && uv run ruff check src/arxiv_archive/minimax_usage.py src/arxiv_archive/minimax_structured.py tests/test_minimax_usage.py tests/test_minimax_structured.py && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M017-cf3fd0/slices/S04/run-evidence/final-m017-guard.json').read_text())
assert g['production_import_allowed'] is False
assert g['ladybugdb_write_allowed'] is False
assert g['minimax_source_of_truth'] is False
assert g['raw_response_persisted'] is False
assert g['exact_quota_values_persisted'] is False
assert g['credential_values_logged'] is False
assert g['raw_corpus_payload_allowed'] is False
print('final-m017-guard-ok')
PY

## Observability Impact

Final guard documents allowed/blocked MiniMax helper boundaries.
