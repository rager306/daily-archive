---
id: T02
parent: S02
milestone: M056-lchpnp
key_files:
  - artifacts/m056-bfs-graph/wave-2/grobid-fulltext/summary.json
  - artifacts/m056-bfs-graph/wave-2/grobid-fulltext/per-pdf/*.json
  - artifacts/m056-bfs-graph/wave-2/grobid-fulltext/tei/*.tei.xml
  - artifacts/m056-bfs-graph/wave-2/opendataloader/summary.json
  - artifacts/m056-bfs-graph/wave-2/opendataloader/per-pdf/*.json
key_decisions:
  - Kept parser commands pointed at http://127.0.0.1:8070 and avoided alternate host-name references.
duration: 
verification_result: passed
completed_at: 2026-06-10T13:39:11.521Z
blocker_discovered: false
---

# T02: Wave 2 parser runs produced 30 GROBID packets and 30 OpenDataLoader packets.

**Wave 2 parser runs produced 30 GROBID packets and 30 OpenDataLoader packets.**

## What Happened

Ran GROBID fulltext against artifacts/m056-bfs-graph/wave-2/corpus-manifest.json using http://127.0.0.1:8070, then ran the OpenDataLoader-only benchmark against the same manifest. GROBID produced 30 successful packets and TEI outputs. OpenDataLoader produced all 30 packets with 28 success, 1 low_quality_source, and 1 opendataloader_unavailable status.

## Verification

Parser summaries show GROBID aggregate_counts success: 30 and packets: 30; OpenDataLoader aggregate_counts success: 28, low_quality_source: 1, opendataloader_unavailable: 1, total packets: 30. Wave 2 tests validate packet counts and safety defaults.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/benchmark_m055deep_grobid_fulltext.py --corpus-manifest artifacts/m056-bfs-graph/wave-2/corpus-manifest.json --output-dir artifacts/m056-bfs-graph/wave-2/grobid-fulltext --grobid-url http://127.0.0.1:8070` | 0 | ✅ pass | 82100ms |
| 2 | `uv run python scripts/benchmark_m055_opendataloader_only.py --corpus-manifest artifacts/m056-bfs-graph/wave-2/corpus-manifest.json --output-dir artifacts/m056-bfs-graph/wave-2/opendataloader` | 0 | ✅ pass | 157400ms |
| 3 | `gsd_exec Summarize M056 Wave 2 parser outputs` | 0 | ✅ pass | 37ms |

## Deviations

OpenDataLoader did not have 30 success statuses; it did produce the required 30 packets. This matches observed runner quality behavior and is documented in analysis.md.

## Known Issues

OpenDataLoader packet quality includes one low_quality_source and one opendataloader_unavailable result.

## Files Created/Modified

- `artifacts/m056-bfs-graph/wave-2/grobid-fulltext/summary.json`
- `artifacts/m056-bfs-graph/wave-2/grobid-fulltext/per-pdf/*.json`
- `artifacts/m056-bfs-graph/wave-2/grobid-fulltext/tei/*.tei.xml`
- `artifacts/m056-bfs-graph/wave-2/opendataloader/summary.json`
- `artifacts/m056-bfs-graph/wave-2/opendataloader/per-pdf/*.json`
