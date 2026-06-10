---
id: T01
parent: S03
milestone: M056-lchpnp
key_files:
  - artifacts/m056-bfs-graph/wave-3/acquisition-log.json
  - artifacts/m056-bfs-graph/wave-3/corpus-manifest.json
  - data/article_catalog/article_catalog/arxiv/*/{arxiv_id}/source/{arxiv_id}.pdf
key_decisions:
  - Reuse the existing parameterized acquisition script without modifying M050-M055deep infrastructure.
duration: 
verification_result: passed
completed_at: 2026-06-10T13:53:41.576Z
blocker_discovered: false
---

# T01: Wave 3 refs 61-90 were acquired from arxiv.org with 30/30 successful PDFs.

**Wave 3 refs 61-90 were acquired from arxiv.org with 30/30 successful PDFs.**

## What Happened

Created a temporary wave-order slice from /tmp/wave-order.json positions 61-90 and ran the existing bounded acquisition script with 3 retries and a 30 second timeout into artifacts/m056-bfs-graph/wave-3. The run wrote acquisition-log.json and corpus-manifest.json and placed PDFs under the local arXiv article catalog by category.

## Verification

Acquisition command completed with success_count=30, blocked_count=0, network_error_count=0, and wrote artifacts/m056-bfs-graph/wave-3/acquisition-log.json.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/acquire_m056_wave.py --wave-order /tmp/wave-3-order.json --wave-order-source --output-dir artifacts/m056-bfs-graph/wave-3 --max-retries 3 --timeout 30 --source-milestone M056-lchpnp/S03 --manifest-schema-version m056-bfs-wave-3-corpus-manifest.v1 --manifest-source-label 'M056-lchpnp S03 Wave 3 acquisition'` | 0 | ✅ pass | 90400ms |

## Deviations

Used /tmp/wave-3-order.json generated from /tmp/wave-order.json positions 61-90 because the existing acquisition script consumes the first 30 IDs from the provided order file in --wave-order-source mode.

## Known Issues

None.

## Files Created/Modified

- `artifacts/m056-bfs-graph/wave-3/acquisition-log.json`
- `artifacts/m056-bfs-graph/wave-3/corpus-manifest.json`
- `data/article_catalog/article_catalog/arxiv/*/{arxiv_id}/source/{arxiv_id}.pdf`
