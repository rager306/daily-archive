---
id: T01
parent: S01
milestone: M056-lchpnp
key_files:
  - scripts/acquire_m056_wave.py
  - artifacts/m056-bfs-graph/wave-1/acquisition-log.json
  - artifacts/m056-bfs-graph/wave-1/corpus-manifest.json
  - data/article_catalog/article_catalog/arxiv/cs-lg/2107.03374/source/2107.03374.pdf
  - data/article_catalog/article_catalog/arxiv/mixed-source/2604.25850/source/2604.25850.pdf
key_decisions:
  - Use the task-explicit Wave 1 ID list as source of truth for S01 acquisition.
  - Keep acquisition fail-closed with all safety defaults false and no graph writes.
duration: 
verification_result: passed
completed_at: 2026-06-10T13:16:31.788Z
blocker_discovered: false
---

# T01: Acquired 30 Wave 1 arXiv PDFs for M056 S01 and wrote the acquisition log plus corpus manifest.

**Acquired 30 Wave 1 arXiv PDFs for M056 S01 and wrote the acquisition log plus corpus manifest.**

## What Happened

Implemented `scripts/acquire_m056_wave.py` as a bounded, fail-closed Wave 1 acquisition script. It uses the task-explicit 30 arXiv IDs, records per-attempt HTTP status and errors, stores PDFs under the article catalog by resolved category, and writes `acquisition-log.json` plus `corpus-manifest.json`. The run acquired 30/30 PDFs with no blocked or network-error entries.

## Verification

Ran `uv run python scripts/acquire_m056_wave.py --wave-order /tmp/wave-order.json --output-dir artifacts/m056-bfs-graph/wave-1 --article-catalog-root data/article_catalog/article_catalog/arxiv --max-retries 3 --timeout 30`; acquisition log reports success=30, blocked=0, network_error=0 and manifest contains 30 PDFs.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/acquire_m056_wave.py --wave-order /tmp/wave-order.json --output-dir artifacts/m056-bfs-graph/wave-1 --article-catalog-root data/article_catalog/article_catalog/arxiv --max-retries 3 --timeout 30` | 0 | ✅ pass | 52500ms |

## Deviations

Used the task-explicit first-30 ID list rather than `/tmp/wave-order.json` because the file's first-mentioned order after self differed from the task contract.

## Known Issues

None.

## Files Created/Modified

- `scripts/acquire_m056_wave.py`
- `artifacts/m056-bfs-graph/wave-1/acquisition-log.json`
- `artifacts/m056-bfs-graph/wave-1/corpus-manifest.json`
- `data/article_catalog/article_catalog/arxiv/cs-lg/2107.03374/source/2107.03374.pdf`
- `data/article_catalog/article_catalog/arxiv/mixed-source/2604.25850/source/2604.25850.pdf`
