---
id: T01
parent: S02
milestone: M058-cmjp1u
key_files:
  - scripts/m058_marker_extract_5.py
  - scripts/m058_marker_compare_5.py
  - artifacts/m058-marker/pilot-5/per-pdf/2603.21520.json
  - artifacts/m058-marker/pilot-5/per-pdf/2605.28617v1.json
  - artifacts/m058-marker/pilot-5/per-pdf/2508.07434.json
  - artifacts/m058-marker/pilot-5/per-pdf/2412.15118.json
  - artifacts/m058-marker/pilot-5/per-pdf/1804.02767.json
  - artifacts/m058-marker/pilot-5/summary.json
  - artifacts/m058-marker/pilot-5/comparison.json
  - artifacts/m058-marker/pilot-5/comparison.md
  - artifacts/m058-marker/pilot-5/decision.md
key_decisions:
  - Use `1804.02767` from M058 S01 as the fifth executable PDF because requested `2305.14314` is absent locally.
  - Record S03 decision as no-go for automatic cumulative 15 expansion because evidence is page-limited and full-document cost is too high.
duration: 
verification_result: passed
completed_at: 2026-06-12T08:12:05.056Z
blocker_discovered: false
---

# T01: Marker stage-1 pilot artifacts, OpenDataLoader comparison, and S03 decision were generated for five executable PDFs.

**Marker stage-1 pilot artifacts, OpenDataLoader comparison, and S03 decision were generated for five executable PDFs.**

## What Happened

Created `scripts/m058_marker_extract_5.py` and `scripts/m058_marker_compare_5.py`. The extract script uses Marker's programmatic PdfConverter API, writes normalized per-PDF JSON packets plus `summary.json`, preserves five false safety defaults, and binds any loopback reference to `127.0.0.1`. The requested `2305.14314` input was not present locally or in the M058 S01 plotextractor summary, so the executable fifth PDF was `1804.02767` from M058 S01; this deviation is recorded in summary and decision artifacts. Full-document and three-page attempts exceeded the command budget before producing the first packet, so the final pilot is explicitly page-limited to page 0 per PDF for bounded cost evidence.

## Verification

Ran `uv run python scripts/m058_marker_extract_5.py` successfully, producing five `marker_extracted` packets and `artifacts/m058-marker/pilot-5/summary.json`. Ran `uv run python scripts/m058_marker_compare_5.py` successfully, producing comparison JSON/Markdown and `decision.md`. Aggregate comparison: avg quality delta 157.5, Marker > ODL 50.0%, decision no-go for automatic S03 due page-limited evidence, missing requested input, and high cost.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/m058_marker_extract_5.py` | 0 | ✅ pass | 2948400ms |
| 2 | `uv run python scripts/m058_marker_compare_5.py` | 0 | ✅ pass | 1000ms |

## Deviations

Requested `2305.14314` was unavailable in local corpus/artifacts; used `1804.02767` from M058 S01 as the fifth executable PDF. Marker was limited to page 0 after full-document and three-page attempts exceeded command budget before producing first packet.

## Known Issues

Full-document Marker cost is too high in the current CPU environment for S03 without a refined strategy. OpenDataLoader comparison data was only available for two of five executable PDFs.

## Files Created/Modified

- `scripts/m058_marker_extract_5.py`
- `scripts/m058_marker_compare_5.py`
- `artifacts/m058-marker/pilot-5/per-pdf/2603.21520.json`
- `artifacts/m058-marker/pilot-5/per-pdf/2605.28617v1.json`
- `artifacts/m058-marker/pilot-5/per-pdf/2508.07434.json`
- `artifacts/m058-marker/pilot-5/per-pdf/2412.15118.json`
- `artifacts/m058-marker/pilot-5/per-pdf/1804.02767.json`
- `artifacts/m058-marker/pilot-5/summary.json`
- `artifacts/m058-marker/pilot-5/comparison.json`
- `artifacts/m058-marker/pilot-5/comparison.md`
- `artifacts/m058-marker/pilot-5/decision.md`
