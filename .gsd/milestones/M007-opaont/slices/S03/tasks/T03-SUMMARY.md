---
id: T03
parent: S03
milestone: M007-opaont
key_files:
  - .gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-summary.json
  - .gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-diagnostics.jsonl
  - .gsd/milestones/M007-opaont/slices/S03/run-evidence/delta-report.json
  - .gsd/milestones/M007-opaont/slices/S03/run-evidence/outlier-report.json
  - .gsd/milestones/M007-opaont/slices/S03/validation-scan-report.md
  - src/arxiv_archive/validation_batch_workflow.py
key_decisions:
  - Persist M007 scan artifacts under S03 run-evidence using M007 names rather than M006 artifact names.
  - Keep M005/S03 structure-aware baseline as the apples-to-apples delta and M005/S06 as mixed benchmark context only.
  - Treat zero import eligibility as a passed import gate; future non-zero eligibility becomes a blocker diagnostic.
duration: 
verification_result: passed
completed_at: 2026-05-20T01:56:24.448Z
blocker_discovered: false
---

# T03: Ran the M007 validation-batch scan dry run and produced scan/delta/outlier evidence.

**Ran the M007 validation-batch scan dry run and produced scan/delta/outlier evidence.**

## What Happened

Ran the bounded validation-batch scan dry run over the S02 30-paper batch state. The CLI produced redacted M007 scan summary/diagnostics, delta report, outlier report, scan response, and updated batch state. The scan matched the M006 evidence: 30 papers, 4,289 chunks, 11 outliers, and zero import-eligible chunks. The delta report correctly separates M005/S03 structure-aware baseline (+2,458 chunks) from M005/S06 mixed benchmark context (+1,818 chunks). No source acquisition, conversion, KG import, or LadybugDB write occurred.

## Verification

Artifact guard confirmed 30 papers, 4,289 chunks, zero import eligibility, correct M005/S03 and M005/S06 deltas, 11 outliers, and no production writes. 31 focused tests passed and ruff passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python -m arxiv_archive validation-batch scan --state-path .gsd/milestones/M007-opaont/slices/S02/run-evidence/batch-state.json --output-dir .gsd/milestones/M007-opaont/slices/S03/run-evidence --structure-baseline-path .gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-summary.json --mixed-benchmark-path .gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-summary.json --json` | 0 | ✅ pass — paper_count=30; chunk_count=4289; outliers=11; import_eligible=0; no writes/import | 4300ms |
| 2 | `test -s .gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-summary.json && uv run python - <<'PY' ... artifact guard ... PY && uv run pytest tests/test_validation_batch_scan_workflow.py tests/test_validation_batch_cli_scan.py tests/test_validation_batch_workflow.py tests/test_validation_batch_cli_preflight.py tests/test_validation_batch_state.py tests/test_thirty_paper_deviation_scan.py -q && uv run ruff check src/arxiv_archive/cli.py src/arxiv_archive/validation_batch_workflow.py src/arxiv_archive/validation_batch_state.py tests/test_validation_batch_scan_workflow.py tests/test_validation_batch_cli_scan.py tests/test_validation_batch_workflow.py` | 0 | ✅ pass — validation-scan-ok; 31 passed; ruff all checks passed | 6600ms |

## Deviations

The first dry-run artifact had an incorrect M005/S06 mixed benchmark delta because the parser did not handle S06's nested `aggregate.total_chunk_count` shape. The parser was fixed, tests were added, and the scan artifacts were regenerated with the correct +1,818 mixed benchmark delta.

## Known Issues

The automated scan repeats M006's 30-paper batch for proof. It does not yet select or run a new +10 batch; that should happen after S04 review confirms the workflow is useful.

## Files Created/Modified

- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-summary.json`
- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-diagnostics.jsonl`
- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/delta-report.json`
- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/outlier-report.json`
- `.gsd/milestones/M007-opaont/slices/S03/validation-scan-report.md`
- `src/arxiv_archive/validation_batch_workflow.py`
