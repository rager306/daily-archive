# M174 Inventory Delta Review

## Verdict

**PASS.** Count movements are explained by exact repair benchmark reclassification with caller-owned-index preserved.

## Category delta

| Category | Baseline | Final | Delta |
|---|---:|---:|---:|
| append-log | 3 | 1 | -2 |
| article-artifact-package | 7 | 7 | +0 |
| caller-owned | 21 | 20 | -1 |
| caller-owned-index | 1 | 1 | +0 |
| database | 1 | 1 | +0 |
| graph-probe-output | 2 | 2 | +0 |
| graph-readiness-evidence | 14 | 14 | +0 |
| legacy-evidence-regeneration | 2 | 2 | +0 |
| parser-replay-output | 3 | 3 | +0 |
| repair-benchmark-output | 0 | 5 | +5 |
| run-owned-state | 1 | 1 | +0 |
| run-scoped | 13 | 11 | -2 |
| script-only | 264 | 264 | +0 |
| source-asset-package | 4 | 4 | +0 |
| source-scan-output | 3 | 3 | +0 |
| temporary | 1 | 1 | +0 |

## Movement rationale

| Movement | Rationale |
|---|---|
| `repair-benchmark-output +5` | Five exact repair benchmark records moved from append-log/run-scoped/caller-owned into one reviewed benchmark-output category. |
| `append-log -2` | Two exact repair diagnostics records moved out; append-log remains for unreviewed diagnostics paths. |
| `run-scoped -2` | Two exact repair summary outputs moved out; run-scoped remains for unreviewed outputs. |
| `caller-owned -1` | One exact repair review output moved out; caller-owned remains for unreviewed caller-provided paths. |
| `caller-owned-index 0` | Existing `chunk_baseline_measurement.py` + `index_path` exception stayed preserved at count 1. |

## Safety checks

- `unknown` remains zero.
- `caller-owned-index=1` remains preserved.
- `caller-owned`, `run-scoped`, and `append-log` remain visible.
- No `shared-state` records were reclassified in M174.
- New category is exact source path matching, not target-word generic.
