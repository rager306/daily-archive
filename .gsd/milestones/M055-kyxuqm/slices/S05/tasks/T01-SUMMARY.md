---
id: T01
parent: S05
milestone: M055-kyxuqm
key_files:
  - scripts/benchmark_m055deep_hybrid_routing_20.py
key_decisions:
  - Treat successful non-low-quality OpenDataLoader markdown above threshold as the body-content winner; use GROBID fulltext fallback when OpenDataLoader is low-quality or unavailable.
duration: 
verification_result: passed
completed_at: 2026-06-10T12:20:02.239Z
blocker_discovered: false
---

# T01: Implemented the 20-PDF GROBID-fulltext versus OpenDataLoader hybrid routing comparator.

**Implemented the 20-PDF GROBID-fulltext versus OpenDataLoader hybrid routing comparator.**

## What Happened

Added scripts/benchmark_m055deep_hybrid_routing_20.py with packet loading, six-dimension comparison, length buckets, data-driven route proposal, residual gap detection, aggregate summary generation, fulltext-versus-header delta, and explicit five-flag false safety defaults.

## Verification

uv run pytest tests/test_m055deep_hybrid_routing_20.py -q passed with 6 tests; the broader regression subset passed with 159 tests; M045/M044 guardrail tests passed with 19 tests.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_m055deep_hybrid_routing_20.py -q` | 0 | ✅ pass: 6 passed | 9000ms |
| 2 | `uv run pytest tests/test_m050_article_artifact_reducer.py tests/test_m050_article_artifact_worker.py tests/test_m050_e2e_pipeline.py tests/test_m052_rlm_workflow.py tests/test_m053_audit_s02.py tests/test_m053_grobid_pilot.py tests/test_m055_benchmark_s01.py tests/test_m055_benchmark_s02.py tests/test_m055_benchmark_s03.py tests/test_m055_benchmark_s04.py tests/test_m055_benchmark_s05.py tests/test_m055deep_corpus_20.py tests/test_m055deep_grobid_fulltext.py tests/test_m055deep_opendataloader_correctness.py tests/test_m055deep_benchmark_20.py tests/test_m055deep_hybrid_routing_20.py -q` | 0 | ✅ pass: 159 passed | 11700ms |
| 3 | `uv run pytest tests/test_m045_project_trajectory.py tests/test_m044_sidecar_architecture_guardrail.py -q` | 0 | ✅ pass: 19 passed | 10100ms |

## Deviations

S05 data did not confirm 100% hybrid. One medium-length PDF with a low-quality OpenDataLoader packet routes to GROBID fulltext only, producing 95% hybrid overall.

## Known Issues

None.

## Files Created/Modified

- `scripts/benchmark_m055deep_hybrid_routing_20.py`
