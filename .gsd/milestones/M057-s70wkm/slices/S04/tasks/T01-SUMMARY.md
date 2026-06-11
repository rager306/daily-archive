---
id: T01
parent: S04
milestone: M057-s70wkm
key_files:
  - scripts/m057_build_graph_manifest.py
  - artifacts/m057-fd-marker/combined-edges.json
  - artifacts/m057-fd-marker/per-layer-summary.json
key_decisions:
  - Normalized citation, table_similarity, and figure_similarity into one diagnostic edge schema.
  - Kept all five safety defaults false in both generated graph artifacts.
duration: null
verification_result: passed
completed_at: 2026-06-11T09:25:12.107Z
blocker_discovered: false
---

# T01: Built the M057 combined graph manifest for citation, table_similarity, and figure_similarity layers.

**Built the M057 combined graph manifest for citation, table_similarity, and figure_similarity layers.**

## What Happened

Added scripts/m057_build_graph_manifest.py to normalize M056 citation edges, M057 table-similarity edges, and M057 figure-similarity edges into one diagnostic schema. Generated combined-edges.json and per-layer-summary.json with explicit five-false safety defaults and 127.0.0.1 local fd context.

## Verification

Ran uv run python scripts/m057_build_graph_manifest.py successfully. Output summary reported 9403 total edges: 4454 citation, 4934 table_similarity, and 15 figure_similarity.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/m057_build_graph_manifest.py` | 0 | ✅ pass | 1000ms |

## Deviations

None.

## Known Issues

Citation layer uses citation_count as similarity_score, so its mean_similarity is count-based and can exceed 1.0; this is documented in REPORT.md.

## Files Created/Modified

- `scripts/m057_build_graph_manifest.py`
- `artifacts/m057-fd-marker/combined-edges.json`
- `artifacts/m057-fd-marker/per-layer-summary.json`
