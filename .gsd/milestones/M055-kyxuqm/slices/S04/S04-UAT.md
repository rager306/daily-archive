# S04: GROBID fulltext plus OpenDataLoader benchmark on 20 PDFs — UAT

**Milestone:** M055-kyxuqm
**Written:** 2026-06-10T12:03:15.615Z

# S04 UAT

## Checks

- PASS: GROBID fulltext probe emitted 20/20 per-PDF JSON packets and summary.json under artifacts/m055deep-parser-benchmark/grobid-fulltext-20.
- PASS: OpenDataLoader probe emitted 20/20 per-PDF JSON packets and summary.json under artifacts/m055deep-parser-benchmark/opendataloader-20.
- PASS: Per-PDF safety defaults remain false for graph_import_allowed, graphdb_written, ladybugdb_written, production_import_attempted, and import_eligible.
- PASS: Aggregate summaries recompute from per-PDF packets in tests/test_m055deep_benchmark_20.py.
- PASS: M045 trajectory verdict is on_track and M044 guardrail pytest exits 0.

## Evidence

- uv run pytest tests/test_m055deep_benchmark_20.py -q => 7 passed.
- M050-M055 regression pytest command => 146 passed.
- uv run pytest tests/test_m045_project_trajectory.py -q && uv run pytest tests/test_m044_sidecar_architecture_guardrail.py -q => 14 passed and 5 passed.

