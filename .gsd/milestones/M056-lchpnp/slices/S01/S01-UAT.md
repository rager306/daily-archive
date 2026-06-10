# S01: Wave 1: first 30 mostmentioned refs of 2605.18747 — UAT

**Milestone:** M056-lchpnp
**Written:** 2026-06-10T13:17:41.228Z

# S01 UAT

- PASS: Acquisition log exists and records 30/30 acquired PDFs, 0 blocked, 0 network errors.
- PASS: GROBID fulltext output contains 30 per-PDF packets and summary success_count=30.
- PASS: OpenDataLoader output contains 30 per-PDF packets and summary success_count=29 with 1 explicit low_quality_source.
- PASS: Wave 1 analysis exists with connectivity new_edge_count=3, self-citation cluster detection, category distribution, and length distribution.
- PASS: Cumulative corpus manifest contains 50 PDFs.
- PASS: Safety defaults remain false; evidence is not authorized for graph import or fact promotion.
- PASS: `uv run pytest tests/test_m056_wave_1.py -q` -> 8 passed.
- PASS: M050-M055deep regression -> 165 passed.
- PASS: M045 trajectory -> on_track; M044 guardrail -> ok.

