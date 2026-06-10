# S02: Wave 2: refs 31-60 — UAT

**Milestone:** M056-lchpnp
**Written:** 2026-06-10T13:39:56.571Z

# S02 UAT

- PASS: Wave 2 acquisition log reports 30 success, 0 blocked, 0 network_error.
- PASS: Wave 2 GROBID summary reports 30 packets and 30 success.
- PASS: Wave 2 OpenDataLoader summary reports 30 packets and 28 success, with 2 documented non-success statuses.
- PASS: Wave 2 analysis reports 2 new edges this wave, 5 cumulative edges, and saturated edge rate versus Wave 1.
- PASS: Cumulative corpus records 80 PDF evidence rows and 77 unique arXiv IDs due to 3 overlaps with Wave 1 explicit IDs.
- PASS: Safety defaults remain false and analysis states evidence is not authorized for graph import or fact promotion.
- PASS: `uv run pytest tests/test_m056_wave_2.py -q` passed 7/7.

