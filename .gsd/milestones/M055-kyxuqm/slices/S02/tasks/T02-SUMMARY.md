---
id: T02
parent: S02
milestone: M055-kyxuqm
key_files:
  - artifacts/m055deep-parser-benchmark/opendataloader-correctness/summary.json
  - artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/1804.02767.json
  - artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/2108.12409.json
  - artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/2109.10862.json
  - artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/2111.00396.json
  - artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/2203.14465.json
key_decisions:
  - Use corpus-manifest.json as the source of the five PDFs: 1804.02767, 2108.12409, 2109.10862, 2111.00396, and 2203.14465.
duration: 
verification_result: passed
completed_at: 2026-06-10T11:45:21.924Z
blocker_discovered: false
---

# T02: Ran OpenDataLoader correctness validation on all five M055 PDFs and emitted per-PDF packets plus summary.

**Ran OpenDataLoader correctness validation on all five M055 PDFs and emitted per-PDF packets plus summary.**

## What Happened

Executed the new correctness probe against artifacts/m055-parser-benchmark/corpus-manifest.json and artifacts/m055-parser-benchmark/opendataloader-only. The run emitted five per-PDF correctness packets under artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf and a deterministic summary.json. All five PDFs completed with status success and error_count 0.

## Verification

uv run python scripts/benchmark_m055deep_opendataloader_correctness.py --corpus-manifest artifacts/m055-parser-benchmark/corpus-manifest.json --opendataloader-dir artifacts/m055-parser-benchmark/opendataloader-only --output-dir artifacts/m055deep-parser-benchmark/opendataloader-correctness exited 0 with success_count 5 and error_count 0. A follow-up file listing confirmed 5 per-PDF JSON packets.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/benchmark_m055deep_opendataloader_correctness.py --corpus-manifest artifacts/m055-parser-benchmark/corpus-manifest.json --opendataloader-dir artifacts/m055-parser-benchmark/opendataloader-only --output-dir artifacts/m055deep-parser-benchmark/opendataloader-correctness` | 0 | ✅ pass: success_count 5, error_count 0 | 4300ms |
| 2 | `find artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf -maxdepth 1 -type f -name '*.json' -print | sort` | 0 | ✅ pass: five per-PDF packets listed | 1000ms |

## Deviations

None.

## Known Issues

charts_detected is 0 for all five real PDFs because no extracted image files are present in the S03 output directory; markdown image references are still counted for figure metrics.

## Files Created/Modified

- `artifacts/m055deep-parser-benchmark/opendataloader-correctness/summary.json`
- `artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/1804.02767.json`
- `artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/2108.12409.json`
- `artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/2109.10862.json`
- `artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/2111.00396.json`
- `artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/2203.14465.json`
