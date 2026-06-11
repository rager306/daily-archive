---
id: S01
parent: M057-s70wkm
milestone: M057-s70wkm
provides:
  - fd validation evidence for downstream graph-readiness gates.
  - 166-row Marker/Nougat fail-closed extraction manifest.
  - OpenDataLoader comparison report for S02/S03 parser decision-making.
requires:
  []
affects:
  - S02
  - S03
key_files:
  - scripts/m057_fd_validate.py
  - scripts/m057_marker_extract.py
  - scripts/m057_compare_marker_opendataloader.py
  - tests/test_m057_s01.py
  - artifacts/m057-fd-marker/fd-validation.json
  - artifacts/m057-fd-marker/marker-extraction/summary.json
  - artifacts/m057-fd-marker/marker-vs-opendataloader.json
  - artifacts/m057-fd-marker/marker-vs-opendataloader.md
key_decisions:
  - Use cumulative-corpus.json as the authoritative 166-PDF manifest.
  - Fail closed with marker_unavailable per PDF when Marker/Nougat preflight fails.
  - Include all explicitly listed OpenDataLoader source directories in comparison.
patterns_established:
  - M057 diagnostic scripts use five explicit false safety defaults.
  - M057 reports use 127.0.0.1 rather than localhost.
observability_surfaces:
  - fd-validation.json records per-test verdicts and latency statistics.
  - marker-extraction/summary.json records install/preflight diagnostics and status counts.
  - marker-vs-opendataloader.json records aggregate and per-PDF comparison metrics.
drill_down_paths:
  - .gsd/milestones/M057-s70wkm/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M057-s70wkm/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M057-s70wkm/slices/S01/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-11T08:07:56.440Z
blocker_discovered: false
---

# S01: fd validation suite + Marker re-extraction on 166 PDFs

**S01 validated fd and produced fail-closed Marker/Nougat extraction plus OpenDataLoader comparison artifacts for the 166-PDF corpus.**

## What Happened

S01 implemented the fd validation suite, the Marker/Nougat extraction driver, and the Marker vs OpenDataLoader comparison pipeline. fd passed 7/7 validation checks against 127.0.0.1 with p50=151.551ms and p95=253.397ms. Marker installation succeeded but marker_single preflight failed; Nougat installation succeeded but nougat preflight failed, so the extractor emitted explicit marker_unavailable packets for all 166 PDFs instead of pretending extraction success. The comparison aligned 166 Marker rows with 165 available OpenDataLoader baselines and emitted JSON and markdown reports.

## Verification

uv run python scripts/m057_fd_validate.py passed 7/7; uv run python scripts/m057_marker_extract.py --per-pdf-timeout-seconds 60 emitted 166 marker_unavailable packets; uv run python scripts/m057_compare_marker_opendataloader.py emitted comparison reports; uv run pytest tests/test_m057_s01.py -q passed 8 tests; uv run python scripts/verify_m044_sidecar_architecture_guardrail.py exited 0.

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

Marker and Nougat were not usable at CLI preflight, so extraction produced fail-closed marker_unavailable packets for all 166 PDFs. The comparison includes all 10 explicitly listed OpenDataLoader source directories, despite prose saying 9 sources.

## Known Limitations

No successful Marker/Nougat PDF extraction occurred in this environment; comparison therefore shows Marker 0.0 average quality and 0.0% Marker better than OpenDataLoader.

## Follow-ups

S02 should either repair the Marker/Nougat runtime or choose a replacement parser before relying on Marker quality evidence.

## Files Created/Modified

- `scripts/m057_fd_validate.py` — New fd validation suite.
- `scripts/m057_marker_extract.py` — New Marker/Nougat extraction driver with fail-closed packets.
- `scripts/m057_compare_marker_opendataloader.py` — New comparison report generator.
- `tests/test_m057_s01.py` — New M057 S01 test coverage.
- `artifacts/m057-fd-marker/` — New fd validation, extraction, and comparison artifacts.
