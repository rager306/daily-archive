---
id: S02
parent: M058-cmjp1u
milestone: M058-cmjp1u
provides:
  - S02 decision evidence for whether to proceed to cumulative 15-PDF Marker S03.
  - Reusable M058 Marker extraction/comparison scripts and tests.
requires:
  []
affects:
  - S03
key_files:
  - scripts/m058_marker_extract_5.py
  - scripts/m058_marker_compare_5.py
  - tests/test_m058_s02.py
  - artifacts/m058-marker/pilot-5/summary.json
  - artifacts/m058-marker/pilot-5/comparison.json
  - artifacts/m058-marker/pilot-5/comparison.md
  - artifacts/m058-marker/pilot-5/decision.md
key_decisions:
  - No-go for automatic S03 expansion because S02 evidence is page-limited and full-document cost is too high.
  - Use `1804.02767` from M058 S01 as the executable fifth PDF because requested `2305.14314` is absent locally.
patterns_established:
  - Marker pilot artifacts carry explicit safety defaults, loopback host, page scope, and per-PDF elapsed seconds.
  - Comparison artifacts distinguish missing ODL data from compared PDFs instead of inventing unavailable ODL metrics.
observability_surfaces:
  - Per-PDF JSON packets include status, counts, package versions, page scope, and elapsed seconds.
  - Aggregate summary and comparison JSON expose sample size, successful extraction count, quality delta, win rate, and go/no-go rationale.
drill_down_paths:
  - .gsd/milestones/M058-cmjp1u/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M058-cmjp1u/slices/S02/tasks/T02-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-12T08:12:51.727Z
blocker_discovered: false
---

# S02: Marker pilot stage 1: 5 PDF + quality eval

**Marker S02 produced five executable page-limited pilot packets, ODL comparison, and a no-go decision for automatic S03 expansion.**

## What Happened

S02 added the M058 Marker extraction and comparison scripts, generated five per-PDF Marker packets under `artifacts/m058-marker/pilot-5/per-pdf`, emitted aggregate `summary.json`, `comparison.json`, `comparison.md`, and `decision.md`, and added seven tests in `tests/test_m058_s02.py`. The requested `2305.14314` was not present in the local corpus or M058 S01 plotextractor output, so `1804.02767` from M058 S01 was used as the fifth executable PDF and the deviation was recorded. Full-document and three-page Marker attempts exceeded the command budget before producing the first packet; the final pilot uses page 0 per PDF to bound S02 cost evidence. The go/no-go decision is no-go for automatic S03 because quality evidence is partial, input readiness is not satisfied, and observed full-document cost is too high.

## Verification

Passed `uv run pytest tests/test_m058_s02.py -q` (7 tests), `uv run pytest tests/test_m045_project_trajectory.py tests/test_m044_sidecar_architecture_guardrail.py -q` (19 tests), `uv run python scripts/check_project_trajectory.py --phase closeout` (on_track), and `uv run python scripts/verify_m044_sidecar_architecture_guardrail.py` (ok). Marker extraction command completed successfully for five executable page-limited PDFs and comparison generation completed successfully.

## Requirements Advanced

None.

## Requirements Validated

None.

## New Requirements Surfaced

- S03 should not proceed until full-document cost and input availability are explicitly resolved.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

Requested `2305.14314` was unavailable; used `1804.02767` from M058 S01. Full-document and three-page Marker attempts exceeded the command budget, so the committed pilot evidence is page-limited to page 0 per PDF.

## Known Limitations

OpenDataLoader data was available for only two of five executable PDFs. Marker full-document CPU cost is not acceptable for automatic cumulative 15-PDF S03 in the current environment.

## Follow-ups

If S03 is revisited, refine the plan first: resolve the intended fifth input, define a full-document cost budget, and consider batching/timeout/resume semantics before running cumulative 15 PDFs.

## Files Created/Modified

- `scripts/m058_marker_extract_5.py` — New Marker pilot extraction script with five false safety defaults and page-limited executable sample.
- `scripts/m058_marker_compare_5.py` — New ODL comparison and decision-generation script.
- `tests/test_m058_s02.py` — New S02 test coverage.
- `artifacts/m058-marker/pilot-5/` — New Marker pilot outputs, comparison reports, and decision doc.
