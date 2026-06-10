---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Wave 2 acquisition collected 30/30 requested arXiv PDFs for refs 31-60.

Reuse scripts/acquire_m056_wave.py with --wave-number 2 (or hardcoded IDs 31-60 from /tmp/wave-order.json). Skip self. Acquire 30 PDFs with bounded retry. Output: artifacts/m056-bfs-graph/wave-2/acquisition-log.json. Accept 25/30 minimum.

## Inputs

- `scripts/acquire_m056_wave.py`

## Expected Output

- `artifacts/m056-bfs-graph/wave-2/acquisition-log.json`

## Verification

test -f artifacts/m056-bfs-graph/wave-2/acquisition-log.json

## Observability Impact

Wave 2 acquisition log.
