---
estimated_steps: 3
estimated_files: 2
skills_used: []
---

# T01: Acquired 30 Wave 1 arXiv PDFs for M056 S01 and wrote the acquisition log plus corpus manifest.

scripts/acquire_m056_wave.py that downloads 30 PDFs from arxiv.org/pdf/{id}. Uses bounded retry (3 attempts, 30s timeout). Input: arxiv ID list (skip self 2605.18747). Reads from /tmp/wave-order.json or hardcoded for Wave 1 (first 30 after self-skip). Output: artifacts/m056-bfs-graph/wave-1/acquisition-log.json with per-PDF status, sha256, attempts, http_status, error.
Accept 25/30 minimum (90% threshold). Document 404s and rate limits.
Per-PDF success/status field: acquired (HTTP 200) / blocked (404) / network_error (timeout).

## Inputs

- `data/article_catalog/article_catalog/arxiv/cs-cl/2605.18747/source/2605.18747.pdf`

## Expected Output

- `artifacts/m056-bfs-graph/wave-1/acquisition-log.json`
- `scripts/acquire_m056_wave.py`
- `30 PDFs in data/article_catalog/article_catalog/arxiv/{cs-cl,cs-cv,cs-lg,cs-ai}/source/`

## Verification

test -f artifacts/m056-bfs-graph/wave-1/acquisition-log.json

## Observability Impact

Acquisition log with sha256, attempts per PDF.
