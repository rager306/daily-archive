---
id: T02
parent: S02
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/chunk_baseline_measurement.py
  - tests/test_chunk_baseline_measurement.py
  - .gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-summary.json
  - .gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-package-diagnostics.jsonl
key_decisions:
  - Current chunks remain valid baseline packages but are not import-ready; all 345 chunks are `ok_for_retrieval_only`.
  - Baseline summary must include explicit refusal reasons and route/state/type counts, not just refused counts.
duration: 
verification_result: passed
completed_at: 2026-05-19T06:22:04.566Z
blocker_discovered: false
---

# T02: Ran the baseline chunk measurement over the gold corpus and proved current chunks are retrieval-only, not import-ready.

**Ran the baseline chunk measurement over the gold corpus and proved current chunks are retrieval-only, not import-ready.**

## What Happened

Ran the S02 baseline builder over the S01 ten-paper gold corpus. The first output had an unhelpful empty refusal_counts despite 345 refused chunks, so the baseline builder was tightened to aggregate package-level refusal reasons and route/state/type counts. The final run produced diagnostics for all 10 papers, with 345 current chunks represented as retrieval-only baseline chunks, 0 import-eligible chunks, 0 import-ready packages, and no raw text, embeddings, production import, or LadybugDB writes.

## Verification

The exact T02 command passed and wrote non-empty baseline summary and diagnostics artifacts. Focused tests and ruff also passed after summary tightening.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_chunk_baseline_measurement.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/chunk_baseline_measurement.py tests/test_chunk_baseline_measurement.py` | 0 | ✅ pass — 23 passed; ruff all checks passed | 8200ms |
| 2 | `uv run python -m arxiv_archive.chunk_baseline_measurement --manifest .gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json --output-dir .gsd/milestones/M005-dlko4z/slices/S02/run-evidence && test -s baseline-summary.json && test -s baseline-package-diagnostics.jsonl` | 0 | ✅ pass — paper_count=10, refused_chunk_count=345, import_ready_count=0, refusal_counts baseline_retrieval_only_not_import_ready=345 | 12000ms |

## Deviations

T02 exposed that the initial baseline summary was count-only: it had 345 refused chunks but empty refusal_counts. The baseline builder was tightened so retrieval-only baseline chunks produce explicit refusal reason `baseline_retrieval_only_not_import_ready` and aggregate route/state/type counts.

## Known Issues

The baseline run reports zero import-ready chunks. This is expected for current PageIndex/SemanticChunk output and confirms S03 must implement graph-grade structure-aware chunks before import rehearsal.

## Files Created/Modified

- `src/arxiv_archive/chunk_baseline_measurement.py`
- `tests/test_chunk_baseline_measurement.py`
- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-package-diagnostics.jsonl`
