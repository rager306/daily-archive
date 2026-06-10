---
estimated_steps: 5
estimated_files: 1
skills_used: []
---

# T02: Ran the 20-PDF hybrid routing comparison and emitted per-PDF plus aggregate artifacts.

Run scripts/benchmark_m055deep_hybrid_routing_20.py against S04 outputs.
- 20/20 per-PDF comparison packets emitted
- summary.json with aggregate routing recommendations
- Per-PDF: grobid_metrics, opendataloader_metrics, comparison_table, recommended_route, residual_gaps, length_bucket
- Aggregate: percent hybrid (expect 100% or close), per-dimension winners, length-bucket patterns

## Inputs

- `scripts/benchmark_m055deep_hybrid_routing_20.py`
- `S04 outputs at grobid-fulltext-20 and opendataloader-20`

## Expected Output

- `artifacts/m055deep-parser-benchmark/hybrid-routing-20/summary.json`

## Verification

test -f artifacts/m055deep-parser-benchmark/hybrid-routing-20/summary.json
