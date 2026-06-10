# S05: Hybrid routing on 20 PDFs with GROBID fulltext comparison — UAT

**Milestone:** M055-kyxuqm
**Written:** 2026-06-10T12:20:21.450Z

# S05 UAT

- PASS: 20 per-PDF routing packets were emitted under `artifacts/m055deep-parser-benchmark/hybrid-routing-20/per-pdf/`.
- PASS: `summary.json` reports schema `m055deep-parser-benchmark.hybrid-routing-20.v1`.
- PASS: Aggregate routing is 95% hybrid with one GROBID-fulltext-only fallback.
- PASS: Five safety defaults remain false and production import is not authorized.
- PASS: Direct S05 tests and regression checks passed.
