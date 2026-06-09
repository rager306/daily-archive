# S04: S04

**Goal:** Run safety guard, independent review where feasible, and finalize recommendation.
**Demo:** After S04, M017 has a final guard and go/no-go recommendation for future KG work.

## Must-Haves

- production_import_allowed=false.
- ladybugdb_write_allowed=false.
- minimax_source_of_truth=false.
- raw payload persistence blocked.
- R045 updated with evidence.

## Proof Level

- This slice proves: Fresh verification plus review artifact.

## Integration Closure

Closes helper implementation before M018 KG candidate locators.

## Verification

- Final no-write/no-import/no-leak guard.

## Tasks

- [x] **T01: Completed final MiniMax helper safety review and validated R045 after remediating security review findings.** `est:small`
  Run fresh verification across MiniMax usage and structured helpers, perform an independent safety review, write final guard and recommendation, and update R045. Verify no production KG import, LadybugDB write, raw response, exact quota, secret, raw corpus, embedding/vector, or source-of-truth path is enabled.
  - Files: `.gsd/milestones/M017-cf3fd0/slices/S04/run-evidence/final-m017-guard.json`, `.gsd/milestones/M017-cf3fd0/slices/S04/run-evidence/m017-independent-review.md`, `.gsd/milestones/M017-cf3fd0/slices/S04/m017-final-recommendation.md`, `.gsd/REQUIREMENTS.md`
  - Verify: uv run pytest tests/test_minimax_usage.py tests/test_minimax_structured.py -q && uv run ruff check src/arxiv_archive/minimax_usage.py src/arxiv_archive/minimax_structured.py tests/test_minimax_usage.py tests/test_minimax_structured.py && uv run python - <<'PY'
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

## Files Likely Touched

- .gsd/milestones/M017-cf3fd0/slices/S04/run-evidence/final-m017-guard.json
- .gsd/milestones/M017-cf3fd0/slices/S04/run-evidence/m017-independent-review.md
- .gsd/milestones/M017-cf3fd0/slices/S04/m017-final-recommendation.md
- .gsd/REQUIREMENTS.md
