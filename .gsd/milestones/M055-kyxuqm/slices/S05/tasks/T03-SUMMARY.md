---
id: T03
parent: S05
milestone: M055-kyxuqm
key_files:
  - tests/test_m055deep_hybrid_routing_20.py
key_decisions:
  - Test expectations follow the S05 length-bucket definition: short 1-10 pages, medium 11-30 pages, long 31+ pages.
duration: 
verification_result: passed
completed_at: 2026-06-10T12:20:02.248Z
blocker_discovered: false
---

# T03: Added S05 hybrid routing tests and completed required regression checks.

**Added S05 hybrid routing tests and completed required regression checks.**

## What Happened

Added tests/test_m055deep_hybrid_routing_20.py with six tests covering 20-PDF output, per-dimension winners, length-bucket patterns, fulltext-versus-header delta, safety defaults, and idempotent summaries. Ran the requested direct test, broader M050/M052/M053/M055/M055deep regression subset, plus M045 trajectory and M044 guardrail tests.

## Verification

uv run pytest tests/test_m055deep_hybrid_routing_20.py -q passed with 6 tests; regression subset passed with 159 tests; M045/M044 passed with 19 tests.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_m055deep_hybrid_routing_20.py -q` | 0 | ✅ pass: 6 passed | 9000ms |
| 2 | `uv run pytest tests/test_m050_article_artifact_reducer.py tests/test_m050_article_artifact_worker.py tests/test_m050_e2e_pipeline.py tests/test_m052_rlm_workflow.py tests/test_m053_audit_s02.py tests/test_m053_grobid_pilot.py tests/test_m055_benchmark_s01.py tests/test_m055_benchmark_s02.py tests/test_m055_benchmark_s03.py tests/test_m055_benchmark_s04.py tests/test_m055_benchmark_s05.py tests/test_m055deep_corpus_20.py tests/test_m055deep_grobid_fulltext.py tests/test_m055deep_opendataloader_correctness.py tests/test_m055deep_benchmark_20.py tests/test_m055deep_hybrid_routing_20.py -q` | 0 | ✅ pass: 159 passed | 11700ms |
| 3 | `uv run pytest tests/test_m045_project_trajectory.py tests/test_m044_sidecar_architecture_guardrail.py -q` | 0 | ✅ pass: 19 passed | 10100ms |

## Deviations

None beyond the data-driven route outcome: 95% hybrid rather than 100%.

## Known Issues

None.

## Files Created/Modified

- `tests/test_m055deep_hybrid_routing_20.py`
