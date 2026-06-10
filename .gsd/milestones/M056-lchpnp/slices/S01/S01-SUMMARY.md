---
id: S01
parent: M056-lchpnp
milestone: M056-lchpnp
provides:
  - 30 acquired Wave 1 PDFs in article catalog.
  - Wave 1 GROBID and OpenDataLoader evidence packets.
  - 50-PDF cumulative corpus manifest for downstream waves.
requires:
  []
affects:
  - S02
  - S03
  - S04
  - S05
  - S06
  - S07
key_files:
  - scripts/acquire_m056_wave.py
  - scripts/analyze_m056_wave_1.py
  - tests/test_m056_wave_1.py
  - artifacts/m056-bfs-graph/wave-1/acquisition-log.json
  - artifacts/m056-bfs-graph/wave-1/corpus-manifest.json
  - artifacts/m056-bfs-graph/wave-1/analysis.md
  - artifacts/m056-bfs-graph/wave-1/cumulative-corpus.json
key_decisions:
  - Use task-explicit Wave 1 ID list as the S01 source of truth.
  - Count Wave 1 connectivity as unique directed edges from Wave 1 TEI references into existing 20-PDF corpus plus anchor.
  - Keep all parser/acquisition outputs as evidence-only artifacts with safety defaults false.
patterns_established:
  - Wave acquisition scripts should emit both acquisition-log.json and corpus-manifest.json for downstream parser probes.
  - Wave analysis should write both machine-readable analysis.json and reader-facing analysis.md.
observability_surfaces:
  - Per-PDF acquisition attempt logs with HTTP status and errors.
  - GROBID and OpenDataLoader per-PDF packets and aggregate summaries.
  - Wave 1 analysis JSON with parser, connectivity, category, length, and self-citation metrics.
drill_down_paths:
  - .gsd/milestones/M056-lchpnp/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M056-lchpnp/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M056-lchpnp/slices/S01/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-10T13:17:41.228Z
blocker_discovered: false
---

# S01: Wave 1: first 30 mostmentioned refs of 2605.18747

**Wave 1 BFS acquisition acquired 30 PDFs, produced parser evidence, and generated a 50-PDF cumulative corpus with connectivity analysis.**

## What Happened

S01 acquired the task-explicit first 30 Wave 1 arXiv references from the 2605.18747 anchor, storing PDFs in the article catalog with per-PDF acquisition metadata. Existing GROBID fulltext and OpenDataLoader probe scripts produced per-PDF packets and summaries. The Wave 1 analysis counted 3 new directed connectivity edges into the existing 20-PDF corpus plus anchor target set, recorded parser quality, self-citation cluster statistics, category distribution, length buckets, and wrote a cumulative 50-PDF corpus manifest.

## Verification

Fresh verification passed: `uv run pytest tests/test_m056_wave_1.py -q` produced 8 passed; M050-M055deep regression produced 165 passed; `uv run python scripts/check_project_trajectory.py --phase closeout` returned verdict=on_track; `uv run python scripts/verify_m044_sidecar_architecture_guardrail.py` returned guardrail ok.

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

Used the task-explicit Wave 1 ID list because `/tmp/wave-order.json` differed from the task-provided first 30. Added anchor GROBID output under Wave 1 artifacts to provide a concrete 2605.18747 TEI for self-citation analysis.

## Known Limitations

OpenDataLoader emitted 1 low_quality_source packet. Self-citation cluster match rate is 0.0% based on direct anchor-citation and first-author overlap signals.

## Follow-ups

S02 can consume `artifacts/m056-bfs-graph/wave-1/cumulative-corpus.json` and Wave 1 parser packets as the current 50-PDF corpus baseline.

## Files Created/Modified

- `scripts/acquire_m056_wave.py` — New bounded Wave 1 PDF acquisition script.
- `scripts/analyze_m056_wave_1.py` — New Wave 1 analysis and cumulative corpus builder.
- `tests/test_m056_wave_1.py` — New artifact tests for Wave 1 evidence.
- `artifacts/m056-bfs-graph/wave-1/` — Wave 1 acquisition, parser, analysis, anchor, and cumulative corpus artifacts.
- `data/article_catalog/article_catalog/arxiv/` — 30 newly acquired Wave 1 PDF source files.
