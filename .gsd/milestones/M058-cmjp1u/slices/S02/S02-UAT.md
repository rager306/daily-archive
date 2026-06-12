# S02: Marker pilot stage 1: 5 PDF + quality eval — UAT

**Milestone:** M058-cmjp1u
**Written:** 2026-06-12T08:12:51.727Z

# S02 UAT

## Checks

- PASS: Five executable Marker per-PDF packets exist with `status=marker_extracted`.
- PASS: `summary.json`, `comparison.json`, `comparison.md`, and `decision.md` exist under `artifacts/m058-marker/pilot-5/`.
- PASS: Five safety defaults are false and loopback host is `127.0.0.1`.
- PASS: M058 S02 tests passed: `uv run pytest tests/test_m058_s02.py -q` => 7 passed.
- PASS: M045 trajectory check is on_track in closeout phase.
- PASS: M044 sidecar architecture guardrail returned ok.
- PASS: Decision recorded as no-go for automatic S03 due page-limited evidence, missing requested input, and high full-document cost.

