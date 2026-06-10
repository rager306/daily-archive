---
id: S02
parent: M055-kyxuqm
milestone: M055-kyxuqm
provides:
  - Structured OpenDataLoader correctness metrics for downstream parser comparison slices.
requires:
  []
affects:
  - S04
  - S05
key_files:
  - scripts/benchmark_m055deep_opendataloader_correctness.py
  - tests/test_m055deep_opendataloader_correctness.py
  - artifacts/m055deep-parser-benchmark/opendataloader-correctness/summary.json
  - artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/1804.02767.json
  - artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/2108.12409.json
  - artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/2109.10862.json
  - artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/2111.00396.json
  - artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/2203.14465.json
key_decisions:
  - Correctness outputs are deterministic and omit generated_at to preserve idempotent summary verification.
  - Missing image directories are treated as zero chart detections rather than parse errors because S03 did not persist extracted image files.
patterns_established:
  - Stdlib-only markdown correctness probe pattern with fail-closed typed diagnostic packets.
  - Per-PDF parser correctness JSON packets include top-level report metrics plus detailed correctness_metrics for drill-down.
observability_surfaces:
  - Per-PDF correctness packets with status and typed diagnostic fields.
  - Aggregate summary.json with per_pdf_statuses, error_count, and safety_defaults.
drill_down_paths:
  - .gsd/milestones/M055-kyxuqm/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M055-kyxuqm/slices/S02/tasks/T02-SUMMARY.md
  - .gsd/milestones/M055-kyxuqm/slices/S02/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-10T11:46:04.721Z
blocker_discovered: false
---

# S02: OpenDataLoader correctness validation on 5 PDFs

**Validated OpenDataLoader correctness beyond counts across five M055 PDFs with structured table, caption, and chart metrics.**

## What Happened

S02 added a deterministic correctness probe for OpenDataLoader markdown output and exercised it against the five-PDF M055 corpus. The probe parses markdown table structures, extracts Figure/Fig./Table captions with line numbers, applies stdlib-only chart heuristics to extracted image files when present, emits fail-closed typed diagnostics on parse errors, and writes per-PDF packets plus an aggregate summary. Real validation completed 5/5 PDFs with zero parse errors.

## Verification

uv run pytest tests/test_m055deep_opendataloader_correctness.py -q passed 10 tests. The final regression command passed 145 tests. The real correctness probe exited 0 with success_count 5 and error_count 0. M045 trajectory returned verdict=on_track phase=closeout, and M044 sidecar architecture guardrail returned ok with exit 0.

## Requirements Advanced

None.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

The script is longer than the approximate 300-line target because image dimension parsing was implemented with stdlib-only code and no new dependencies. No separate M051/M054 pytest files exist, so the available M050/M052/M053/M055/M055deep regression set was run.

## Known Limitations

Real chart detection returned zero because the S03 OpenDataLoader output has markdown image references but no persisted image files. Several OpenDataLoader markdown tables are one-row or empty-image grid structures, which lowers the table_structure_quality_score.

## Follow-ups

Downstream slices should account for OpenDataLoader markdown producing many caption references and some low-structure table blocks; if future OpenDataLoader runs persist images, the chart detection path will start reporting chart types.

## Files Created/Modified

- `scripts/benchmark_m055deep_opendataloader_correctness.py` — New correctness probe for OpenDataLoader markdown, captions, image chart heuristics, per-PDF packets, and summary output.
- `tests/test_m055deep_opendataloader_correctness.py` — New 10-test suite for table parsing, caption extraction, chart detection, aggregate probing, safety defaults, idempotency, and fail-closed diagnostics.
- `artifacts/m055deep-parser-benchmark/opendataloader-correctness/` — New real-run correctness summary and five per-PDF packets.
