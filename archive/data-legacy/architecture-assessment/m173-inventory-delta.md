# M173 Inventory Delta Review

## Verdict

**PASS.** Count movements are explained by exact path-family reclassification only.

## Category delta

| Category | Baseline | Final | Delta |
|---|---:|---:|---:|
| append-log | 3 | 3 | +0 |
| article-artifact-package | 7 | 7 | +0 |
| caller-owned | 28 | 21 | -7 |
| caller-owned-index | 1 | 1 | +0 |
| database | 1 | 1 | +0 |
| graph-probe-output | 0 | 2 | +2 |
| graph-readiness-evidence | 14 | 14 | +0 |
| legacy-evidence-regeneration | 2 | 2 | +0 |
| parser-replay-output | 0 | 3 | +3 |
| run-owned-state | 1 | 1 | +0 |
| run-scoped | 14 | 13 | -1 |
| script-only | 264 | 264 | +0 |
| source-asset-package | 4 | 4 | +0 |
| source-scan-output | 0 | 3 | +3 |
| temporary | 1 | 1 | +0 |

## Movement rationale

| Movement | Rationale |
|---|---|
| `parser-replay-output +3` | Exact records in `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` moved from caller-owned/run-scoped into one reviewed replay-output category. |
| `source-scan-output +3` | Exact records in approved thirty-paper source scan modules moved from caller-owned into one reviewed scan-output category. |
| `graph-probe-output +2` | Exact records in `src/research_graph/infrastructure/graph/r024_networkx_probe.py` moved from caller-owned into one reviewed graph-probe category. |
| `caller-owned -7` | Seven exact approved records moved out; broad caller-owned remains for unreviewed paths. |
| `run-scoped -1` | One exact parser replay output moved out; broad run-scoped remains for unreviewed output/artifact paths. |

## Safety checks

- `unknown` remains zero.
- `caller-owned`, `run-scoped`, and `append-log` remain visible.
- No `shared-state` records were reclassified in M173.
- New categories are exact source path-family matches, not target-word generic.
