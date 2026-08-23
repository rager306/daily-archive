# M177 Markdown Cache Policy Review

## Verdict

**No scanner movement in M177.** Markdown converter writes remain `caller-owned` because the baseline records are explicit caller/adapter-owned output paths, not shared cache state.

## Reviewed records

| Source path | Records | Current category | Targets | Decision |
|---|---:|---|---|---|
| `src/research_graph/infrastructure/corpus/sources/markdown_converter.py` | 2 | `caller-owned` | `md_path`, `method_path` | preserve |

## Rationale

- The targets are caller-visible output path variables, not stable shared cache indexes.
- Reclassifying by generic words such as `cache`, `md_path`, or `method_path` would violate the M172-M177 exact path-family rule.
- Cache-like paths remain conservative unless exact shared-state ownership is proven.
- Preserving `caller-owned` keeps `shared-state=0` meaningful and avoids hiding future cache risk.

## Regression requirement

Focused tests must assert that markdown converter `md_path` and `method_path` remain `caller-owned`.

## Follow-up

If future markdown conversion adds stable shared cache files or indexes, create a dedicated cache-coordination milestone with exact path and lifecycle review before scanner reclassification.
