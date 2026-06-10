# S03: Wave 3: refs 61-90 — UAT

**Milestone:** M056-lchpnp
**Written:** 2026-06-10T14:00:13.640Z

# S03 UAT

## Checks

- PASS: Wave 3 acquisition log exists and records 30 acquired PDFs, 0 blocked, 0 network errors.
- PASS: GROBID fulltext produced 30 per-PDF packets and 30 successes using http://127.0.0.1:8070.
- PASS: OpenDataLoader produced 30 per-PDF packets, with 29 success and 1 opendataloader_unavailable diagnostic packet.
- PASS: Wave 3 analysis reports edge saturation by wave as Wave 1 = 3, Wave 2 = 2, Wave 3 = 1, cumulative directed edges = 6.
- PASS: The analysis markdown includes the required safety statement: This evidence is not authorized for graph import or fact promotion.
- PASS: Regression tests for Wave 1 and Wave 2 remained green.

## Evidence

- `artifacts/m056-bfs-graph/wave-3/acquisition-log.json`
- `artifacts/m056-bfs-graph/wave-3/grobid-fulltext/summary.json`
- `artifacts/m056-bfs-graph/wave-3/opendataloader/summary.json`
- `artifacts/m056-bfs-graph/wave-3/analysis.md`
- Final verification: `uv run pytest tests/test_m056_wave_3.py tests/test_m056_wave_2.py tests/test_m056_wave_1.py && uv run python scripts/check_project_trajectory.py --phase closeout && uv run python scripts/verify_m044_sidecar_architecture_guardrail.py`

