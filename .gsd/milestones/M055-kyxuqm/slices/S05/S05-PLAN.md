# S05: Hybrid routing on 20 PDFs with GROBID fulltext comparison

**Goal:** Re-run hybrid routing analysis on 20 PDFs using GROBID fulltext vs OpenDataLoader. Compare with M055 S04 (5 PDFs, header-only GROBID). Per-dimension winners across 20 PDFs. Identify if any PDF recommends GROBID-only or OpenDataLoader-only. Surface corpus-level patterns: short PDFs vs long PDFs.
**Demo:** 20-PDF hybrid routing packets with per-dimension winners and fulltext-vs-header delta.

## Must-Haves

- 20/20 per-PDF comparison packets emitted
- summary.json with aggregate routing recommendations
- Per-PDF: grobid_metrics, opendataloader_metrics, comparison_table, recommended_route, length_bucket
- 6+ tests pass
- 5 safety defaults stay false
- M045 trajectory on_track, M044 guardrail exit 0
- 1 commit in git history

## Proof Level

- This slice proves: operational

## Integration Closure

20-PDF hybrid routing with fulltext-aware decision.

## Verification

- Per-PDF comparison + length-bucket patterns + routing distribution emitted.

## Tasks

- [x] **T01: Implemented the 20-PDF GROBID-fulltext versus OpenDataLoader hybrid routing comparator.** `est:60m`
  scripts/benchmark_m055deep_hybrid_routing_20.py (~350 lines) mirroring M055 S04 but for 20 PDFs. Functions:
  - _load_packets(per_pdf_dir) -> dict[arxiv_id, packet]
  - _compare_dimensions(grobid_packet, opendataloader_packet) -> dict: 6 dimensions as before + length_bucket (short/medium/long based on pages)
  - _propose_route(comparison) -> dict: returns hybrid or single-parser based on data
  - _identify_residual_gaps(comparison) -> list[dict]
  - compare_hybrid_routing_20(grobid_dir, opendataloader_dir, output_dir) -> dict
    - schema_version: m055deep-parser-benchmark.hybrid-routing-20.v1
    - per-PDF comparison packets at artifacts/m055deep-parser-benchmark/hybrid-routing-20/per-pdf/{arxiv_id}.json
    - summary.json with: aggregate routing recommendation, percent hybrid, per-dimension winners, length-bucket patterns, fulltext_vs_header_delta (vs M055 S04)
  - CLI: --grobid-dir, --opendataloader-dir, --output-dir
  - 5-flag safety defaults explicit (all false)
  - Idempotent (modulo generated_at)
  - IMPORTANT: do NOT hardcode hybrid recommendation. If GROBID fulltext dominates a dimension for 20 PDFs, route to GROBID. Use per-dimension threshold.
  - Files: `scripts/benchmark_m055deep_hybrid_routing_20.py`
  - Verify: test -f scripts/benchmark_m055deep_hybrid_routing_20.py

- [x] **T02: Ran the 20-PDF hybrid routing comparison and emitted per-PDF plus aggregate artifacts.** `est:10m`
  Run scripts/benchmark_m055deep_hybrid_routing_20.py against S04 outputs.
  - 20/20 per-PDF comparison packets emitted
  - summary.json with aggregate routing recommendations
  - Per-PDF: grobid_metrics, opendataloader_metrics, comparison_table, recommended_route, residual_gaps, length_bucket
  - Aggregate: percent hybrid (expect 100% or close), per-dimension winners, length-bucket patterns
  - Files: `artifacts/m055deep-parser-benchmark/hybrid-routing-20/summary.json`
  - Verify: test -f artifacts/m055deep-parser-benchmark/hybrid-routing-20/summary.json

- [x] **T03: Added S05 hybrid routing tests and completed required regression checks.** `est:25m`
  tests/test_m055deep_hybrid_routing_20.py with 6+ tests:
  1. test_20_pdfs_routing
  2. test_per_dimension_winners
  3. test_length_bucket_patterns
  4. test_fulltext_vs_header_delta
  5. test_5_safety_defaults_all_false
  6. test_idempotent_summary
  7. M050+M051+M052+M053+M054 S01-S05 regression: all still pass
  - Files: `tests/test_m055deep_hybrid_routing_20.py`
  - Verify: uv run pytest tests/test_m055deep_hybrid_routing_20.py -q

## Files Likely Touched

- scripts/benchmark_m055deep_hybrid_routing_20.py
- artifacts/m055deep-parser-benchmark/hybrid-routing-20/summary.json
- tests/test_m055deep_hybrid_routing_20.py
