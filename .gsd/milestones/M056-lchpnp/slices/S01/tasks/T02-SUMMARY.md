---
id: T02
parent: S01
milestone: M056-lchpnp
key_files:
  - artifacts/m056-bfs-graph/wave-1/grobid-fulltext/summary.json
  - artifacts/m056-bfs-graph/wave-1/grobid-fulltext/per-pdf/2107.03374.json
  - artifacts/m056-bfs-graph/wave-1/grobid-fulltext/tei/2107.03374.tei.xml
  - artifacts/m056-bfs-graph/wave-1/opendataloader/summary.json
  - artifacts/m056-bfs-graph/wave-1/opendataloader/per-pdf/2107.03374.json
key_decisions:
  - Reuse the existing M055deep GROBID and M055 OpenDataLoader probe scripts without modifying them.
  - Treat low_quality_source as explicit parser-quality evidence, not as a graph-import authorization.
duration: 
verification_result: passed
completed_at: 2026-06-10T13:16:47.235Z
blocker_discovered: false
---

# T02: Ran GROBID fulltext and OpenDataLoader probes over the 30 acquired Wave 1 PDFs.

**Ran GROBID fulltext and OpenDataLoader probes over the 30 acquired Wave 1 PDFs.**

## What Happened

Executed the existing M055deep GROBID fulltext probe and the existing M055 OpenDataLoader-only probe against the Wave 1 corpus manifest. GROBID produced 30 per-PDF packets with 30 successes. OpenDataLoader produced 30 per-PDF packets with 29 successes and 1 low_quality_source packet, preserving explicit fail-closed quality evidence.

## Verification

Ran the planned GROBID and OpenDataLoader commands. GROBID summary reports total=30, success=30, blocked=0, low_quality=0, body_positive=30, ref_positive=30. OpenDataLoader summary reports total=30, success=29, low_quality_source=1, opendataloader_unavailable=0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/benchmark_m055deep_grobid_fulltext.py --corpus-manifest artifacts/m056-bfs-graph/wave-1/corpus-manifest.json --output-dir artifacts/m056-bfs-graph/wave-1/grobid-fulltext --grobid-url http://127.0.0.1:8070` | 0 | ✅ pass | 98300ms |
| 2 | `uv run python scripts/benchmark_m055_opendataloader_only.py --corpus-manifest artifacts/m056-bfs-graph/wave-1/corpus-manifest.json --output-dir artifacts/m056-bfs-graph/wave-1/opendataloader` | 0 | ✅ pass | 105300ms |

## Deviations

None.

## Known Issues

One OpenDataLoader packet is low_quality_source; this is recorded in summary and analysis rather than hidden.

## Files Created/Modified

- `artifacts/m056-bfs-graph/wave-1/grobid-fulltext/summary.json`
- `artifacts/m056-bfs-graph/wave-1/grobid-fulltext/per-pdf/2107.03374.json`
- `artifacts/m056-bfs-graph/wave-1/grobid-fulltext/tei/2107.03374.tei.xml`
- `artifacts/m056-bfs-graph/wave-1/opendataloader/summary.json`
- `artifacts/m056-bfs-graph/wave-1/opendataloader/per-pdf/2107.03374.json`
