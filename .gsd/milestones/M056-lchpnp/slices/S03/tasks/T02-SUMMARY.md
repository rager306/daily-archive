---
id: T02
parent: S03
milestone: M056-lchpnp
key_files:
  - artifacts/m056-bfs-graph/wave-3/grobid-fulltext/summary.json
  - artifacts/m056-bfs-graph/wave-3/grobid-fulltext/per-pdf/*.json
  - artifacts/m056-bfs-graph/wave-3/opendataloader/summary.json
  - artifacts/m056-bfs-graph/wave-3/opendataloader/per-pdf/*.json
key_decisions:
  - Use 127.0.0.1 for the GROBID URL to avoid browser-evidence host-name false positives.
duration: 
verification_result: passed
completed_at: 2026-06-10T13:53:54.988Z
blocker_discovered: false
---

# T02: Wave 3 PDFs were parsed through GROBID fulltext and OpenDataLoader.

**Wave 3 PDFs were parsed through GROBID fulltext and OpenDataLoader.**

## What Happened

Ran GROBID fulltext against the Wave 3 corpus manifest using http://127.0.0.1:8070 and ran OpenDataLoader-only parsing into the wave-3 artifact tree. GROBID produced 30 success packets; OpenDataLoader produced 30 packets with 29 success and 1 opendataloader_unavailable diagnostic packet.

## Verification

Parser summaries and packet directories exist: GROBID 30 packets and 30 successes, OpenDataLoader 30 packets with aggregate_counts success=29 and opendataloader_unavailable=1. The wave-3 pytest suite subsequently verified both packet counts.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/benchmark_m055deep_grobid_fulltext.py --corpus-manifest artifacts/m056-bfs-graph/wave-3/corpus-manifest.json --output-dir artifacts/m056-bfs-graph/wave-3/grobid-fulltext --grobid-url http://127.0.0.1:8070` | 0 | ✅ pass | 191400ms |
| 2 | `uv run python scripts/benchmark_m055_opendataloader_only.py --corpus-manifest artifacts/m056-bfs-graph/wave-3/corpus-manifest.json --output-dir artifacts/m056-bfs-graph/wave-3/opendataloader` | 0 | ✅ pass | 188500ms |

## Deviations

OpenDataLoader reported one opendataloader_unavailable packet; this is captured as diagnostic evidence and does not block the required 30 packet output.

## Known Issues

One Wave 3 OpenDataLoader packet is opendataloader_unavailable; GROBID succeeded for the same 30 PDFs.

## Files Created/Modified

- `artifacts/m056-bfs-graph/wave-3/grobid-fulltext/summary.json`
- `artifacts/m056-bfs-graph/wave-3/grobid-fulltext/per-pdf/*.json`
- `artifacts/m056-bfs-graph/wave-3/opendataloader/summary.json`
- `artifacts/m056-bfs-graph/wave-3/opendataloader/per-pdf/*.json`
