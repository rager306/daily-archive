---
id: T01
parent: S02
milestone: M055-kyxuqm
key_files:
  - scripts/benchmark_m055deep_opendataloader_correctness.py
key_decisions:
  - Use deterministic outputs without generated_at so summary.json is idempotent across repeated correctness runs.
  - Treat missing image directories as zero chart detections rather than an error because S03 markdown references images but did not persist image files.
duration: 
verification_result: passed
completed_at: 2026-06-10T11:45:07.056Z
blocker_discovered: false
---

# T01: Implemented the OpenDataLoader correctness validation script for table structure, captions, and chart heuristics.

**Implemented the OpenDataLoader correctness validation script for table structure, captions, and chart heuristics.**

## What Happened

Created scripts/benchmark_m055deep_opendataloader_correctness.py. The script parses markdown tables with alignment separators, extracts Figure/Fig./Table captions with line numbers, detects chart-like extracted images with stdlib-only SVG/PNG/GIF/JPEG heuristics, emits deterministic per-PDF packets and summary.json, and keeps all five safety defaults false. Parse failures fail closed into typed diagnostic packets.

## Verification

uv run pytest tests/test_m055deep_opendataloader_correctness.py -q passed 10 tests; uv run python -m py_compile scripts/benchmark_m055deep_opendataloader_correctness.py tests/test_m055deep_opendataloader_correctness.py passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_m055deep_opendataloader_correctness.py -q` | 0 | ✅ pass: 10 passed in 0.16s | 25100ms |
| 2 | `uv run python -m py_compile scripts/benchmark_m055deep_opendataloader_correctness.py tests/test_m055deep_opendataloader_correctness.py` | 0 | ✅ pass | 1000ms |

## Deviations

Script is longer than the approximate 300-line target because image dimension parsing was implemented without adding dependencies.

## Known Issues

OpenDataLoader S03 did not persist image files next to markdown, so real chart detection returns zero charts while retaining functional heuristics for future runs and tests.

## Files Created/Modified

- `scripts/benchmark_m055deep_opendataloader_correctness.py`
