---
estimated_steps: 13
estimated_files: 1
skills_used: []
---

# T01: Implemented the 20-PDF GROBID-fulltext versus OpenDataLoader hybrid routing comparator.

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

## Inputs

- `artifacts/m055deep-parser-benchmark/grobid-fulltext-20/per-pdf/*.json`
- `artifacts/m055deep-parser-benchmark/opendataloader-20/per-pdf/*.json`

## Expected Output

- `scripts/benchmark_m055deep_hybrid_routing_20.py`

## Verification

test -f scripts/benchmark_m055deep_hybrid_routing_20.py

## Observability Impact

20-PDF comparison + length-bucket patterns + routing distribution emitted.
