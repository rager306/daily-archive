# M172 Impact Analysis and Test Plan

## Pre-edit GitNexus impact

GitNexus impact was attempted before editing scanner code.

| Target | Result | Risk | Impacted count |
|---|---|---|---:|
| `_classify` | target not found | UNKNOWN | 0 |
| `scripts/inventory_write_paths.py` | target not found | UNKNOWN | 0 |

## Blast radius statement

The pre-edit blast radius is **UNKNOWN** because GitNexus could not resolve the scanner function or file target. This is not proof of safety. M172 proceeds only because the planned diff is local to `scripts/inventory_write_paths.py`, changes only category labels/reasons, and will be covered by focused tests plus final `gitnexus_detect_changes`.

## Code surface

Scanner category logic is centralized in `_classify(source_path, operation, target, mode)`. M172 should add exact source-path checks there and avoid changes to AST traversal, record schema, or output rendering.

## Focused test plan

Positive tests:

1. `src/research_graph/infrastructure/graph/readiness/export.py` with `summary_path` returns `graph-readiness-evidence`.
2. `src/research_graph/infrastructure/papers/source_assets/registry.py` with source asset package output returns `source-asset-package`.
3. `src/research_graph/cli/commands/article_artifacts.py` or `src/research_graph/infrastructure/papers/artifacts/...` returns `article-artifact-package`.

Fallback tests:

1. Unapproved path with `summary_path` remains `caller-owned` or `run-scoped` according to existing rules.
2. Unapproved path with `index_path` stays `shared-state` unless it is an existing exact reviewed exception.
3. Generic queue/state/catalog words remain conservative.

## Edit constraints

- Do not change record schema.
- Do not change traversal.
- Do not add dependencies.
- Do not use broad target-name regexes for approved categories.
- Keep exact path checks before broad fallback rules only where necessary.
