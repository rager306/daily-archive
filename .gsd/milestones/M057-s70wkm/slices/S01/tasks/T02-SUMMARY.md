---
id: T02
parent: S01
milestone: M057-s70wkm
key_files:
  - scripts/m057_marker_extract.py
  - artifacts/m057-fd-marker/marker-extraction/summary.json
  - artifacts/m057-fd-marker/marker-extraction/per-pdf/
key_decisions:
  - Use artifacts/m056-bfs-graph/cumulative-corpus.json as the authoritative 166-PDF manifest.
  - Fail closed with marker_unavailable per PDF when Marker and Nougat are not usable.
duration: 
verification_result: passed
completed_at: 2026-06-11T08:07:12.717Z
blocker_discovered: false
---

# T02: Implemented Marker/Nougat extraction driver and generated fail-closed packets for all 166 PDFs.

**Implemented Marker/Nougat extraction driver and generated fail-closed packets for all 166 PDFs.**

## What Happened

Created scripts/m057_marker_extract.py. The script loads the 166-PDF M056 cumulative corpus, attempts marker-pdf installation and marker_single preflight, attempts nougat-ocr installation and nougat preflight when Marker is unusable, and emits one per-PDF JSON packet for every corpus PDF. In this environment Marker and Nougat CLIs both failed preflight, so the run used backend none and emitted status=marker_unavailable for all 166 PDFs rather than fabricating extraction success.

## Verification

uv run python scripts/m057_marker_extract.py --per-pdf-timeout-seconds 60 completed with total_pdfs=166 and status_counts={marker_unavailable:166}; summary written to artifacts/m057-fd-marker/marker-extraction/summary.json.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/m057_marker_extract.py --per-pdf-timeout-seconds 60` | 0 | ✅ pass | 18000ms |

## Deviations

Marker/Nougat extraction was unavailable at CLI preflight, so the slice produced fail-closed marker_unavailable packets instead of successful table extraction.

## Known Issues

Marker preflight exits non-zero after marker-pdf install; Nougat preflight exits non-zero after nougat-ocr install. No PDF content extraction succeeded in this environment.

## Files Created/Modified

- `scripts/m057_marker_extract.py`
- `artifacts/m057-fd-marker/marker-extraction/summary.json`
- `artifacts/m057-fd-marker/marker-extraction/per-pdf/`
