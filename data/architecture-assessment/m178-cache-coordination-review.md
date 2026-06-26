# M178 Cache Coordination Review

## Verdict

**No scanner movement in M178.** Cache-like and markdown-like records are already typed by exact ownership where reviewed, or remain conservative script-only where unreviewed. No broad cache category is added.

## Reviewed cache-like records

| Source path | Target | Current category | Decision |
|---|---|---|---|
| `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` | `cache_path` | `parser-replay-output` | preserve exact reviewed category |
| `src/research_graph/infrastructure/corpus/reporting/coverage_report.py` | `self.markdown_path` | `caller-owned` | preserve caller-owned |
| `src/research_graph/infrastructure/corpus/sources/markdown_converter.py` | `md_path`, `method_path` | `caller-owned` | preserve caller-owned |
| `src/research_graph/infrastructure/papers/artifacts/metrics.py` | `markdown_path` | `article-artifact-package` | preserve exact reviewed category |
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | `index_path` | `caller-owned-index` | preserve paired index category |

## Script markdown-like no-move records

Several scripts still write markdown outputs and remain `script-only`, including audit, M052, M060, render, and test-architecture scripts. These should be reviewed by future exact family waves, not by a broad markdown/cache rule.

## Coordination policy

- Caller-provided markdown outputs remain `caller-owned`.
- Reviewed package/report outputs keep their exact package categories.
- Stable shared cache or index files require explicit lifecycle review before classification.
- Broad target-name classification by `cache_path`, `markdown_path`, `md_path`, or `index_path` is forbidden.
- `shared-state=0` must remain meaningful; do not hide cache risk by reclassifying unreviewed stable paths.

## Regression coverage

Existing focused tests assert markdown converter `md_path` and `method_path` remain `caller-owned`. M178 reruns those tests as the cache-coordination proof.

## Upgrade path

If a future stable shared cache/index appears, create a dedicated cache-coordination milestone that documents owner, lifecycle, invalidation behavior, concurrency behavior, and failure mode before scanner movement.
