---
id: S04
parent: M055-kyxuqm
milestone: M055-kyxuqm
provides:
  - 20-PDF GROBID fulltext baseline for S05.
  - 20-PDF OpenDataLoader baseline for S05.
  - Automated S04 artifact contract tests.
requires:
  - slice: S03
    provides: corpus-manifest-20.json and 20 local PDFs.
affects:
  - S05
key_files:
  - scripts/benchmark_m055deep_grobid_fulltext.py
  - tests/test_m055deep_benchmark_20.py
  - artifacts/m055deep-parser-benchmark/grobid-fulltext-20/summary.json
  - artifacts/m055deep-parser-benchmark/opendataloader-20/summary.json
key_decisions:
  - Keep OpenDataLoader output backward-compatible by normalizing opendataloader_unavailable to blocked only in S04 tests.
  - Add GROBID sections as a non-breaking per-PDF diagnostic field while preserving section_count.
patterns_established:
  - 20-PDF benchmark artifact tests recompute summaries from per-PDF packets instead of trusting stored aggregates.
  - Cross-parser aggregate status comparison can normalize parser-specific blocked labels without mutating existing outputs.
observability_surfaces:
  - Per-PDF status packets for both parsers.
  - Aggregate parser summaries for status and content counts.
  - S04 UAT artifact with verification commands.
drill_down_paths:
  - .gsd/milestones/M055-kyxuqm/slices/S04/tasks/T01-SUMMARY.md
  - .gsd/milestones/M055-kyxuqm/slices/S04/tasks/T02-SUMMARY.md
  - .gsd/milestones/M055-kyxuqm/slices/S04/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-10T12:03:15.615Z
blocker_discovered: false
---

# S04: GROBID fulltext plus OpenDataLoader benchmark on 20 PDFs

**Benchmarked GROBID fulltext and OpenDataLoader across the expanded 20-PDF corpus with per-PDF packets, aggregate summaries, and regression tests.**

## What Happened

S04 executed both parser probes against artifacts/m055deep-parser-benchmark/corpus-manifest-20.json. GROBID fulltext ran on all 20 PDFs through http://127.0.0.1:8070/api/processFulltextDocument and produced per-PDF JSON plus TEI outputs under grobid-fulltext-20. OpenDataLoader ran on all 20 PDFs and produced per-PDF JSON plus markdown/layout artifacts under opendataloader-20. A new S04 test file verifies 20/20 coverage, aggregate status counts, safety defaults, summary recomputation from per-PDF packets, required per-PDF fields, and manifest alignment. Required regression, trajectory, and guardrail checks passed.

## Verification

Passed: uv run pytest tests/test_m055deep_benchmark_20.py -q (7 passed); passed M050-M055 regression command (146 passed); passed uv run pytest tests/test_m045_project_trajectory.py -q and uv run pytest tests/test_m044_sidecar_architecture_guardrail.py -q (14 passed, 5 passed); trajectory-report.json verdict is on_track.

## Requirements Advanced

None.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

Added the required GROBID per-PDF sections field before finalizing artifacts; OpenDataLoader keeps its existing opendataloader_unavailable status name and the S04 tests normalize that as blocked for cross-parser aggregate assertions.

## Known Limitations

OpenDataLoader emitted one low_quality_source packet in the 20-PDF corpus; no parser was blocked. The existing OpenDataLoader run() deprecation warning remains unchanged.

## Follow-ups

S05 can consume grobid-fulltext-20 and opendataloader-20 baselines for hybrid routing/reporting.

## Files Created/Modified

- `scripts/benchmark_m055deep_grobid_fulltext.py` — Added sections list to GROBID per-PDF metrics.
- `tests/test_m055deep_benchmark_20.py` — Added seven S04 20-PDF benchmark artifact tests.
- `artifacts/m055deep-parser-benchmark/grobid-fulltext-20/` — Generated GROBID fulltext 20-PDF benchmark artifacts.
- `artifacts/m055deep-parser-benchmark/opendataloader-20/` — Generated OpenDataLoader 20-PDF benchmark artifacts.
