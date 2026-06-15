---
id: T01
parent: S01
milestone: M067-oqsavh
key_files:
  - artifacts/m066-graphdb-reselection/candidates/falkordb-report.md
  - artifacts/m066-graphdb-reselection/scoring-matrix.md
  - artifacts/m066-graphdb-reselection/distribution-model.md
  - tests/test_m067_s01.py
  - .gsd/milestones/M067-oqsavh/slices/S01/S01-PLAN.md
  - .gsd/milestones/M067-oqsavh/slices/S01/tasks/T01-SUMMARY.md
key_decisions:
  - FalkorDB license score is 4/5 for self-hosted daily-archive under SSPLv1.
  - FalkorDB becomes the self-hosted winner at 70/90; Neo4j remains highest total at 76/90 but is not selected for the self-hosted ranking due to AGPLv3 risk.
duration:
verification_result: passed
completed_at: 2026-06-15T07:19:18.905Z
blocker_discovered: false
---

# T01: Re-scored FalkorDB to 70/90 under corrected SSPLv1 self-hosted distribution model.

**Re-scored FalkorDB to 70/90 under corrected SSPLv1 self-hosted distribution model.**

## What Happened

Updated the FalkorDB candidate report to correct the license model from the M066 confusion to SSPLv1, documented the daily-archive self-hosted research distribution assumption, and updated the M066 scoring matrix to distinguish total-score leadership from the M067 self-hosted selection ranking. Added M067 S01 contract tests covering the corrected license, distribution model, self-hosted viability, updated scoring matrix, five safety defaults, loopback/local-service gotcha handling, and M045/M044/M050-M066 regression anchors.

## Verification

uv run pytest tests/test_m067_s01.py -q passed 7 tests. uv run pytest tests/test_m045_project_trajectory.py tests/test_m044_sidecar_architecture_guardrail.py tests/test_m044_live_grobid_candidate_probe.py -q passed 24 tests.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_m067_s01.py -q` | 0 | ✅ pass: 7 passed in 0.14s | 9700ms |
| 2 | `uv run pytest tests/test_m045_project_trajectory.py tests/test_m044_sidecar_architecture_guardrail.py tests/test_m044_live_grobid_candidate_probe.py -q` | 0 | ✅ pass: 24 passed in 0.28s | 15300ms |

## Deviations

None.

## Known Issues

Existing unrelated working-tree modifications were present before staging; only M067 S01 files and GSD task state are staged for this commit.

## Files Created/Modified

- `artifacts/m066-graphdb-reselection/candidates/falkordb-report.md`
- `artifacts/m066-graphdb-reselection/scoring-matrix.md`
- `artifacts/m066-graphdb-reselection/distribution-model.md`
- `tests/test_m067_s01.py`
- `.gsd/milestones/M067-oqsavh/slices/S01/S01-PLAN.md`
- `.gsd/milestones/M067-oqsavh/slices/S01/tasks/T01-SUMMARY.md`
