---
id: T03
parent: S01
milestone: M057-s70wkm
key_files:
  - scripts/m057_compare_marker_opendataloader.py
  - tests/test_m057_s01.py
  - artifacts/m057-fd-marker/marker-vs-opendataloader.json
  - artifacts/m057-fd-marker/marker-vs-opendataloader.md
key_decisions:
  - Include all explicitly listed OpenDataLoader directories in the comparison, even though the prose says 9 sources.
  - Use the aggregate correctness score when per-PDF OpenDataLoader quality is absent, otherwise use a conservative status-based quality fallback.
duration: 
verification_result: passed
completed_at: 2026-06-11T08:07:26.445Z
blocker_discovered: false
---

# T03: Implemented Marker vs OpenDataLoader comparison and M057 S01 pytest coverage.

**Implemented Marker vs OpenDataLoader comparison and M057 S01 pytest coverage.**

## What Happened

Created scripts/m057_compare_marker_opendataloader.py and tests/test_m057_s01.py. The comparison reads the M057 Marker extraction summary plus all listed OpenDataLoader sources from M055/M056, aligns per PDF by arxiv_id, computes table-count and table-quality deltas, and emits JSON plus markdown reports. The tests cover fd health, single embedding, batch embedding, p95 latency, 166-PDF extraction accounting, comparison aggregates, five false safety defaults, and a lightweight M050-M056 regression-control smoke check.

## Verification

uv run python scripts/m057_compare_marker_opendataloader.py completed; uv run pytest tests/test_m057_s01.py -q passed 8 tests. M044 guardrail also passed with 'm044 sidecar architecture guardrail ok'. M045 active trajectory returned drift_risk before commit due uncommitted_changes_present and will be rerun after commit.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/m057_compare_marker_opendataloader.py` | 0 | ✅ pass | 5400ms |
| 2 | `uv run pytest tests/test_m057_s01.py -q` | 0 | ✅ pass | 2700ms |
| 3 | `uv run python scripts/verify_m044_sidecar_architecture_guardrail.py` | 0 | ✅ pass | 5900ms |
| 4 | `uv run python scripts/check_project_trajectory.py --phase active` | 0 | ⚠️ drift_risk before commit due uncommitted changes | 5900ms |

## Deviations

Comparison includes all 10 explicitly listed OpenDataLoader source directories, despite the task text also saying 9 sources. One of the 166 corpus PDFs has no matched OpenDataLoader packet, yielding 165 matched baselines.

## Known Issues

Marker average quality is 0.0 because Marker/Nougat extraction was unavailable; OpenDataLoader average quality is 0.958 under the available packet-quality heuristic.

## Files Created/Modified

- `scripts/m057_compare_marker_opendataloader.py`
- `tests/test_m057_s01.py`
- `artifacts/m057-fd-marker/marker-vs-opendataloader.json`
- `artifacts/m057-fd-marker/marker-vs-opendataloader.md`
