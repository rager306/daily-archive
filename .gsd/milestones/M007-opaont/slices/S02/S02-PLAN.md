# S02: Batch initialization and source preflight

**Goal:** Implement deterministic validation batch initialization and source preflight using the S01 state schema, producing redacted batch state and source readiness diagnostics without scan execution or production writes.
**Demo:** After this slice, a batch can be initialized and source-preflighted with redacted readiness/contradiction artifacts, without production writes or unbounded repair.

## Must-Haves

- CLI can initialize a validation batch from a manifest with deterministic selected paper records.
- CLI can run source preflight and update batch state.
- Source readiness distinguishes Markdown present/accepted, PDF present/missing, conversion repaired/failed, unavailable source, and Markdown-scan readiness.
- Contradictions from S01 diagnostics are persisted.
- Commands write only local redacted batch artifacts.
- No acquisition, scan execution, KG import, or LadybugDB writes occur in S02.

## Proof Level

- This slice proves: Focused tests plus dry-run CLI over M006 corpus manifest or fixtures.

## Integration Closure

Consumes S01 contract/state helpers and prepares batch artifacts that S03 scan automation can consume. Uses existing corpus manifests/source path conventions without running unbounded conversion or scan work.

## Verification

- Adds persisted batch-state.json, selection manifest, source-preflight summary, and source-preflight diagnostics JSONL with contradiction flags and safety fields.

## Tasks

- [x] **T01: Implement batch workflow preflight helpers** `est:medium`
  Implement pure batch workflow helpers for batch directory layout, manifest loading, deterministic initialization, source path inspection, source preflight summaries, diagnostics JSONL writing, and state updates. Keep this module free of real conversion/acquisition/scanning.
  - Files: `src/arxiv_archive/validation_batch_workflow.py`, `tests/test_validation_batch_workflow.py`
  - Verify: uv run pytest tests/test_validation_batch_workflow.py tests/test_validation_batch_state.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py src/arxiv_archive/validation_batch_state.py tests/test_validation_batch_workflow.py tests/test_validation_batch_state.py

- [x] **T02: Wire batch init and preflight CLI** `est:medium`
  Wire `validation-batch init` and `validation-batch preflight` to the new workflow helpers. `init` should create batch-state and selection manifest. `preflight` should update source readiness and diagnostics. Leave scan/review/resume as non-zero stubs.
  - Files: `src/arxiv_archive/cli.py`, `tests/test_validation_batch_cli_preflight.py`
  - Verify: uv run pytest tests/test_validation_batch_workflow.py tests/test_validation_batch_cli_contract.py tests/test_validation_batch_cli_preflight.py tests/test_analysis.py -q && uv run ruff check src/arxiv_archive/cli.py src/arxiv_archive/validation_batch_workflow.py src/arxiv_archive/validation_batch_state.py tests/test_validation_batch_workflow.py tests/test_validation_batch_cli_contract.py tests/test_validation_batch_cli_preflight.py

- [x] **T03: Run bounded source preflight dry run** `est:medium`
  Run a bounded dry-run batch using the existing M006 30-paper corpus manifest into M007 batch artifacts. Verify 30 selected papers, source-preflight summary, diagnostics, and no scan/import/write flags.
  - Files: `.gsd/milestones/M007-opaont/slices/S02/run-evidence/batch-state.json`, `.gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-summary.json`, `.gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-diagnostics.jsonl`, `.gsd/milestones/M007-opaont/slices/S02/source-preflight-report.md`
  - Verify: test -s .gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
p=Path('.gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-summary.json')
s=json.loads(p.read_text())
assert s['paper_count']==30
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('source-preflight-ok')
PY

## Files Likely Touched

- src/arxiv_archive/validation_batch_workflow.py
- tests/test_validation_batch_workflow.py
- src/arxiv_archive/cli.py
- tests/test_validation_batch_cli_preflight.py
- .gsd/milestones/M007-opaont/slices/S02/run-evidence/batch-state.json
- .gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-summary.json
- .gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-diagnostics.jsonl
- .gsd/milestones/M007-opaont/slices/S02/source-preflight-report.md
