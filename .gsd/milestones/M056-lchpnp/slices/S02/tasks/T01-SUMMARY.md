---
id: T01
parent: S02
milestone: M056-lchpnp
key_files:
  - scripts/acquire_m056_wave.py
  - artifacts/m056-bfs-graph/wave-2/acquisition-log.json
  - artifacts/m056-bfs-graph/wave-2/corpus-manifest.json
  - data/article_catalog/article_catalog/arxiv/*/*/source/*.pdf
key_decisions:
  - Used a temporary Wave 2 order file derived from /tmp/wave-order.json positions 31-60 to preserve the existing Wave 1 default behavior.
  - Added explicit acquisition manifest metadata parameters so Wave 2 artifacts are labeled M056-lchpnp/S02 without breaking S01 defaults.
duration: 
verification_result: passed
completed_at: 2026-06-10T13:38:53.437Z
blocker_discovered: false
---

# T01: Wave 2 acquisition collected 30/30 requested arXiv PDFs for refs 31-60.

**Wave 2 acquisition collected 30/30 requested arXiv PDFs for refs 31-60.**

## What Happened

Built /tmp/m056-wave-2-order.json from positions 31-60 of /tmp/wave-order.json and ran scripts/acquire_m056_wave.py with the existing bounded retry/timeout behavior. The first run downloaded PDFs; after discovering the manifest metadata still defaulted to S01, the acquisition script was parameterized for source milestone/schema/source label and rerun against existing PDFs to regenerate Wave 2 acquisition-log.json and corpus-manifest.json with S02 metadata.

## Verification

Acquisition summary shows requested 30, success 30, blocked 0, network_error 0; corpus manifest has 30 PDFs from 2508.04289 through 2602.05842; safety defaults all false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/acquire_m056_wave.py --wave-order /tmp/m056-wave-2-order.json --wave-order-source --output-dir artifacts/m056-bfs-graph/wave-2 --source-milestone M056-lchpnp/S02 --manifest-schema-version m056-bfs-wave-2-corpus-manifest.v1 --manifest-source-label 'M056-lchpnp S02 Wave 2 acquisition'` | 0 | ✅ pass | 17100ms |
| 2 | `gsd_exec Summarize M056 Wave 2 final counts after rerun` | 0 | ✅ pass | 44ms |

## Deviations

Parameterized scripts/acquire_m056_wave.py metadata instead of hardcoding Wave 2 IDs. Three Wave 2 refs overlap prior Wave 1 explicit IDs because Wave 1 used a task override order that differs from /tmp/wave-order.json.

## Known Issues

Wave 2 positions 31-60 include 2603.03836, 2603.05621, and 2603.19329, which were already acquired by Wave 1's explicit override list; cumulative row count is 80 while unique arXiv ID count is 77.

## Files Created/Modified

- `scripts/acquire_m056_wave.py`
- `artifacts/m056-bfs-graph/wave-2/acquisition-log.json`
- `artifacts/m056-bfs-graph/wave-2/corpus-manifest.json`
- `data/article_catalog/article_catalog/arxiv/*/*/source/*.pdf`
