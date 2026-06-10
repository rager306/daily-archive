---
id: T02
parent: S04
milestone: M055-kyxuqm
key_files:
  - artifacts/m055deep-parser-benchmark/opendataloader-20/summary.json
  - artifacts/m055deep-parser-benchmark/opendataloader-20/per-pdf/*.json
  - artifacts/m055deep-parser-benchmark/opendataloader-20/markdown/*.md
  - artifacts/m055deep-parser-benchmark/opendataloader-20/layout/*.json
key_decisions:
  - Use the existing --corpus-manifest CLI path in benchmark_m055_opendataloader_only.py instead of adding a wrapper.
duration: 
verification_result: passed
completed_at: 2026-06-10T12:02:27.773Z
blocker_discovered: false
---

# T02: Ran the OpenDataLoader probe across the 20-PDF corpus and emitted 20 per-PDF packets plus aggregate summary.

**Ran the OpenDataLoader probe across the 20-PDF corpus and emitted 20 per-PDF packets plus aggregate summary.**

## What Happened

Executed scripts/benchmark_m055_opendataloader_only.py against artifacts/m055deep-parser-benchmark/corpus-manifest-20.json with output under artifacts/m055deep-parser-benchmark/opendataloader-20. The existing CLI already supported --corpus-manifest, so no wrapper or CLI change was needed. The run produced 20 per-PDF JSON packets, markdown/layout artifacts, and summary.json.

## Verification

uv run python scripts/benchmark_m055_opendataloader_only.py --corpus-manifest artifacts/m055deep-parser-benchmark/corpus-manifest-20.json --output-dir artifacts/m055deep-parser-benchmark/opendataloader-20 exited 0; summary reported 20 total PDFs with 19 success, 1 low_quality_source, and 0 opendataloader_unavailable.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/benchmark_m055_opendataloader_only.py --corpus-manifest artifacts/m055deep-parser-benchmark/corpus-manifest-20.json --output-dir artifacts/m055deep-parser-benchmark/opendataloader-20` | 0 | ✅ pass | 83100ms |
| 2 | `uv run python - <<'PY'
import json
from pathlib import Path
summary=json.loads(Path('artifacts/m055deep-parser-benchmark/opendataloader-20/summary.json').read_text())
print(summary['total_pdfs'], summary['aggregate_counts'])
print(len(list(Path('artifacts/m055deep-parser-benchmark/opendataloader-20/per-pdf').glob('*.json'))))
PY` | 0 | ✅ pass | 1000ms |

## Deviations

None.

## Known Issues

The script emits an existing DeprecationWarning for OpenDataLoader run(); this did not block output generation and was not changed in this slice.

## Files Created/Modified

- `artifacts/m055deep-parser-benchmark/opendataloader-20/summary.json`
- `artifacts/m055deep-parser-benchmark/opendataloader-20/per-pdf/*.json`
- `artifacts/m055deep-parser-benchmark/opendataloader-20/markdown/*.md`
- `artifacts/m055deep-parser-benchmark/opendataloader-20/layout/*.json`
