# S03: S03

**Goal:** Automate validation-batch scan execution over preflighted batch state, producing redacted scan artifacts, baseline deltas, outlier gates, and import-safety status for review.
**Demo:** After this slice, a batch can run the existing deviation scanner and produce redacted delta/outlier reports against previous, cumulative, and M005 baselines.

## Must-Haves

- CLI can run validation-batch scan from source-ready batch state.
- Scan command writes redacted scan summary and diagnostics.
- Delta report separates M005/S03 structure-aware baseline from M005/S06 mixed benchmark context.
- Outlier report uses documented thresholds and includes normalized density when available.
- Any non-zero import eligibility is surfaced as a blocker unless explicitly reviewed.
- No raw/chunk text, embeddings/vectors, KG import, or LadybugDB writes occur.

## Proof Level

- This slice proves: Focused tests plus bounded dry-run scan over the existing 30-paper batch artifacts.

## Integration Closure

Consumes S02 batch-state/source-preflight artifacts and existing M006 deviation scanner. Produces batch scan/delta/outlier artifacts that S04 can independently review.

## Verification

- Adds validation-scan summary/diagnostics, delta report, outlier report, and import gate status under M007 batch evidence.

## Tasks

- [x] **T01: Implemented batch scan workflow helpers and redacted delta/outlier artifact generation.** `est:medium`
  Extend workflow helpers with scan orchestration around the existing thirty-paper deviation scanner. The helper should build scan inputs from batch state, call redacted scanner logic, write validation-scan artifacts, and update batch phase without importing KG facts or writing LadybugDB.
  - Files: `src/arxiv_archive/validation_batch_workflow.py`, `tests/test_validation_batch_scan_workflow.py`
  - Verify: uv run pytest tests/test_validation_batch_scan_workflow.py tests/test_validation_batch_workflow.py tests/test_thirty_paper_deviation_scan.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py tests/test_validation_batch_scan_workflow.py

- [x] **T02: Wired validation-batch scan CLI to redacted scan/delta/outlier artifact generation.** `est:medium`
  Wire `validation-batch scan` to the scan workflow helper. It should require a source-ready state path, write scan/delta/outlier artifacts, and keep review/resume as non-zero stubs.
  - Files: `src/arxiv_archive/cli.py`, `tests/test_validation_batch_cli_scan.py`
  - Verify: uv run pytest tests/test_validation_batch_cli_scan.py tests/test_validation_batch_cli_preflight.py tests/test_validation_batch_cli_contract.py tests/test_analysis.py -q && uv run ruff check src/arxiv_archive/cli.py src/arxiv_archive/validation_batch_workflow.py tests/test_validation_batch_cli_scan.py

- [x] **T03: Ran the M007 validation-batch scan dry run and produced scan/delta/outlier evidence.** `est:medium`
  Run bounded scan dry run over the S02 30-paper batch state and write M007 scan/delta/outlier artifacts plus a short report. Verify zero import eligibility, no production writes, and expected 4,289 chunks.
  - Files: `.gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-summary.json`, `.gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-diagnostics.jsonl`, `.gsd/milestones/M007-opaont/slices/S03/run-evidence/delta-report.json`, `.gsd/milestones/M007-opaont/slices/S03/run-evidence/outlier-report.json`, `.gsd/milestones/M007-opaont/slices/S03/validation-scan-report.md`
  - Verify: test -s .gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-summary.json').read_text())
assert s['paper_count']==30
assert s['aggregate']['chunk_count']==4289
assert s['aggregate']['import_eligible_chunk_count']==0
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('validation-scan-ok')
PY

## Files Likely Touched

- src/arxiv_archive/validation_batch_workflow.py
- tests/test_validation_batch_scan_workflow.py
- src/arxiv_archive/cli.py
- tests/test_validation_batch_cli_scan.py
- .gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-summary.json
- .gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-diagnostics.jsonl
- .gsd/milestones/M007-opaont/slices/S03/run-evidence/delta-report.json
- .gsd/milestones/M007-opaont/slices/S03/run-evidence/outlier-report.json
- .gsd/milestones/M007-opaont/slices/S03/validation-scan-report.md
