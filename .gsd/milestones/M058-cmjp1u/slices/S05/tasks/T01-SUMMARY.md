---
id: T01
parent: S05
milestone: M058-cmjp1u
key_files:
  - scripts/m058_build_graph_manifest.py
  - artifacts/m058-pilot/combined-edges.json
  - artifacts/m058-pilot/per-layer-summary.json
key_decisions:
  - Keep citation mean_similarity as null because citation relation is not numeric similarity.
  - Keep all five safety defaults explicitly false in both generated manifests.
duration: 
verification_result: passed
completed_at: 2026-06-12T08:27:48.142Z
blocker_discovered: false
---

# T01: Built the M058 combined four-layer graph manifest.

**Built the M058 combined four-layer graph manifest.**

## What Happened

Added scripts/m058_build_graph_manifest.py and generated artifacts/m058-pilot/combined-edges.json plus artifacts/m058-pilot/per-layer-summary.json. The manifest combines M056 citation edges, M057 table similarity, M057 figure similarity v1, and M058 plotextractor figure similarity v2 into a normalized diagnostic schema with explicit five-false safety defaults and 127.0.0.1 loopback metadata.

## Verification

Ran uv run python scripts/m058_build_graph_manifest.py successfully; output reported 9418 edges across 4 layers and wrote both expected artifacts.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/m058_build_graph_manifest.py` | 0 | ✅ pass | 5700ms |

## Deviations

None.

## Known Issues

Citation edges do not carry similarity scores, so citation mean_similarity is null rather than synthetic.

## Files Created/Modified

- `scripts/m058_build_graph_manifest.py`
- `artifacts/m058-pilot/combined-edges.json`
- `artifacts/m058-pilot/per-layer-summary.json`
