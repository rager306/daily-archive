# S03: Quota fill gate and scan new plus ten batch

**Goal:** Add a quota-fill gate for the new +10 batch, prove the current batch reaches 10 accepted source-ready papers, then run validation-batch scan only if the quota gate passes.
**Demo:** After this slice, the new +10 batch has a quota-fill gate artifact proving accepted_ready_count=10 before scan, then scan/delta/outlier artifacts, or a clear blocker if quota cannot be filled.

## Must-Haves

- Quota-fill artifact records target_count, attempted_count, accepted_count, rejected_count, shortage_count, and accepted_ready_count.
- Scan runs only when accepted_ready_count equals target_count and shortage_count is zero.
- Current M008 batch proves attempted_count=10, accepted_ready_count=10, rejected_count=0, shortage_count=0.
- Scan artifacts are redacted and safety flags false.
- Non-zero import eligibility blocks progression.
- PDF incompleteness remains documented as caveat.

## Proof Level

- This slice proves: Quota-fill artifact guard plus validation-batch scan artifact guard and focused tests.

## Integration Closure

Consumes S02 final source-ready batch state and S01 candidate inventory. Produces quota-fill artifact, scan artifacts, and report for S04 review.

## Verification

- Adds quota-fill summary/diagnostics before scan, then scan/delta/outlier artifacts with explicit import gate status.

## Tasks

- [x] **T01: Implement quota fill gate helpers** `est:medium`
  Implement quota-fill helper functions and tests. The helper should classify source-ready selected papers as accepted, mark unready papers as rejected/needs_replacement, and compute shortage_count. It should support deterministic future replacement metadata but not perform unbounded acquisition in this task.
  - Files: `src/arxiv_archive/validation_batch_workflow.py`, `tests/test_validation_batch_quota_fill.py`
  - Verify: uv run pytest tests/test_validation_batch_quota_fill.py tests/test_validation_batch_workflow.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py tests/test_validation_batch_quota_fill.py

- [x] **T02: Write current batch quota fill artifact** `est:small`
  Generate quota-fill summary and diagnostics for the current M008 new +10 batch from the final S02 preflight state. Because current batch is 10/10 ready, no replacements should be needed, but the artifact must prove that before scan.
  - Files: `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-summary.json`, `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-diagnostics.jsonl`
  - Verify: test -s .gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-summary.json').read_text())
assert s['target_count']==10
assert s['accepted_ready_count']==10
assert s['shortage_count']==0
assert s['raw_text_included'] is False
print('quota-fill-ok')
PY

- [x] **T03: Run quota-gated validation scan** `est:medium`
  Run validation-batch scan over the quota-filled S02 state and write scan/delta/outlier artifacts plus a scan report. Include quota-fill evidence and PDF caveat in the report.
  - Files: `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/validation-scan-summary.json`, `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/validation-scan-diagnostics.jsonl`, `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/delta-report.json`, `.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/outlier-report.json`, `.gsd/milestones/M008-c9zb94/slices/S03/validation-scan-report.md`
  - Verify: test -s .gsd/milestones/M008-c9zb94/slices/S03/run-evidence/validation-scan-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
q=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-summary.json').read_text())
s=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S03/run-evidence/validation-scan-summary.json').read_text())
assert q['accepted_ready_count']==10
assert s['paper_count']==10
assert s['aggregate']['import_eligible_chunk_count']==0
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('quota-gated-scan-ok')
PY

## Files Likely Touched

- src/arxiv_archive/validation_batch_workflow.py
- tests/test_validation_batch_quota_fill.py
- .gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-summary.json
- .gsd/milestones/M008-c9zb94/slices/S03/run-evidence/quota-fill-diagnostics.jsonl
- .gsd/milestones/M008-c9zb94/slices/S03/run-evidence/validation-scan-summary.json
- .gsd/milestones/M008-c9zb94/slices/S03/run-evidence/validation-scan-diagnostics.jsonl
- .gsd/milestones/M008-c9zb94/slices/S03/run-evidence/delta-report.json
- .gsd/milestones/M008-c9zb94/slices/S03/run-evidence/outlier-report.json
- .gsd/milestones/M008-c9zb94/slices/S03/validation-scan-report.md
