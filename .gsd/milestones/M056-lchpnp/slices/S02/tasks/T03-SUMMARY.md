---
id: T03
parent: S02
milestone: M056-lchpnp
key_files:
  - scripts/analyze_m056_wave_2.py
  - tests/test_m056_wave_2.py
  - artifacts/m056-bfs-graph/wave-2/analysis.json
  - artifacts/m056-bfs-graph/wave-2/analysis.md
  - artifacts/m056-bfs-graph/wave-2/cumulative-corpus.json
key_decisions:
  - Cumulative corpus actual_total follows the slice contract's PDF evidence row count of 80, while unique_arxiv_id_count documents the 77 unique-ID overlap reality.
  - Wave 2 edge saturation compares Wave 2's new directed edge count to Wave 1's 3 directed edges against the same 20-PDF plus anchor target set.
duration: 
verification_result: passed
completed_at: 2026-06-10T13:39:29.904Z
blocker_discovered: false
---

# T03: Wave 2 analysis, tests, regression checks, trajectory check, and guardrail check all passed.

**Wave 2 analysis, tests, regression checks, trajectory check, and guardrail check all passed.**

## What Happened

Created scripts/analyze_m056_wave_2.py to read Wave 2 acquisition, Wave 2 parser packets, Wave 1 packets, the 20-PDF existing corpus, and the anchor TEI. The script emits Wave 2 analysis JSON/Markdown and cumulative corpus evidence. Added tests/test_m056_wave_2.py with seven artifact checks covering acquisition minimums, parser packet counts, edge saturation, cumulative corpus count, safety defaults, parser quality, and self-citation cluster reporting. Ran targeted Wave 2 tests, M050-M055deep regression tests, M044/M045 tests, and direct trajectory/guardrail scripts.

## Verification

uv run pytest tests/test_m056_wave_2.py -q passed 7/7. M050-M055deep regression passed 165/165. M044/M045 pytest passed 19/19. Direct scripts reported trajectory verdict=on_track and m044 sidecar architecture guardrail ok.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/analyze_m056_wave_2.py` | 0 | ✅ pass | 180000ms |
| 2 | `uv run pytest tests/test_m056_wave_2.py -q` | 0 | ✅ pass | 3400ms |
| 3 | `uv run pytest tests/test_m050_article_artifact_reducer.py tests/test_m050_article_artifact_worker.py tests/test_m050_e2e_pipeline.py tests/test_m052_rlm_workflow.py tests/test_m053_audit_s02.py tests/test_m053_grobid_pilot.py tests/test_m055_benchmark_s01.py tests/test_m055_benchmark_s02.py tests/test_m055_benchmark_s03.py tests/test_m055_benchmark_s04.py tests/test_m055_benchmark_s05.py tests/test_m055deep_benchmark_20.py tests/test_m055deep_corpus_20.py tests/test_m055deep_grobid_fulltext.py tests/test_m055deep_hybrid_routing_20.py tests/test_m055deep_opendataloader_correctness.py tests/test_m055deep_report_s06.py -q` | 0 | ✅ pass | 33500ms |
| 4 | `uv run pytest tests/test_m044_sidecar_architecture_guardrail.py tests/test_m045_project_trajectory.py -q` | 0 | ✅ pass | 10600ms |
| 5 | `uv run python scripts/check_project_trajectory.py && uv run python scripts/verify_m044_sidecar_architecture_guardrail.py` | 0 | ✅ pass | 120000ms |

## Deviations

Cumulative corpus records 80 PDF evidence rows and separately reports 77 unique arXiv IDs because three Wave 2 refs overlap Wave 1 explicit IDs.

## Known Issues

OpenDataLoader Wave 2 success count is 28/30, with two non-success packet statuses documented in analysis.

## Files Created/Modified

- `scripts/analyze_m056_wave_2.py`
- `tests/test_m056_wave_2.py`
- `artifacts/m056-bfs-graph/wave-2/analysis.json`
- `artifacts/m056-bfs-graph/wave-2/analysis.md`
- `artifacts/m056-bfs-graph/wave-2/cumulative-corpus.json`
