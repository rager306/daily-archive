# S02: OpenDataLoader correctness validation on 5 PDFs — UAT

**Milestone:** M055-kyxuqm
**Written:** 2026-06-10T11:46:04.721Z

# S02 UAT: OpenDataLoader correctness validation

## Checks

- PASS: Correctness script exists at `scripts/benchmark_m055deep_opendataloader_correctness.py` and parses markdown tables, captions, chart-like images, deterministic summary output, and fail-closed diagnostics.
- PASS: Real run emitted 5/5 per-PDF packets under `artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/` plus `summary.json`.
- PASS: Aggregate summary reports `tables_total=55`, `tables_with_caption=42`, `figures_total=71`, `figures_with_caption=43`, `charts_detected=0`, `table_structure_quality_score=0.426`, `image_caption_rate=0.606`.
- PASS: Safety defaults remain false in summary and per-PDF packets.
- PASS: `uv run pytest tests/test_m055deep_opendataloader_correctness.py -q` passed 10/10.
- PASS: Final regression command passed 145/145.
- PASS: M045 trajectory verdict is `on_track`; M044 guardrail exited 0.

## Notes

OpenDataLoader S03 markdown references images, but S03 did not persist actual extracted image files in its output tree, so chart detection is correctly zero on the real corpus while remaining tested with synthetic PNG fixtures.
