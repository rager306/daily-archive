# M172 Inventory Delta Review

## Verdict

**PASS.** Count movements are explained by exact path-family reclassification only.

## Category delta

| Category | Baseline | Final | Delta |
|---|---:|---:|---:|
| append-log | 7 | 3 | -4 |
| article-artifact-package | 0 | 7 | +7 |
| caller-owned | 38 | 28 | -10 |
| caller-owned-index | 1 | 1 | +0 |
| database | 1 | 1 | +0 |
| graph-readiness-evidence | 0 | 14 | +14 |
| legacy-evidence-regeneration | 2 | 2 | +0 |
| run-owned-state | 1 | 1 | +0 |
| run-scoped | 25 | 14 | -11 |
| script-only | 264 | 264 | +0 |
| source-asset-package | 0 | 4 | +4 |
| temporary | 1 | 1 | +0 |

## Movement rationale

| Movement | Rationale |
|---|---|
| `graph-readiness-evidence +14` | Exact records under `src/research_graph/infrastructure/graph/readiness/` moved from caller-owned/run-scoped/append-log into one reviewed evidence category. |
| `source-asset-package +4` | Exact records in `src/research_graph/infrastructure/papers/source_assets/registry.py` moved into one reviewed package-output category. |
| `article-artifact-package +7` | Exact records in article artifact CLI/infrastructure paths moved into one reviewed package-output category. |
| `caller-owned -10` | Ten exact approved records moved out; broad caller-owned remains for unreviewed paths. |
| `run-scoped -11` | Eleven exact approved records moved out; broad run-scoped remains for unreviewed output/artifact paths. |
| `append-log -4` | Four exact approved append-like records moved out; append-log remains for unreviewed event/diagnostics paths. |

## Safety checks

- `unknown` remains zero.
- `caller-owned`, `run-scoped`, and `append-log` remain visible.
- No `shared-state` records were reclassified in M172.
- New categories are path-family exact, not target-word generic.
