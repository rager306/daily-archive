# M174 Impact Analysis and Test Plan

## Pre-edit GitNexus impact

GitNexus impact was attempted before editing scanner code.

| Target | Result | Risk | Impacted count |
|---|---|---|---:|
| `_classify` | target not found | UNKNOWN | 0 |
| `scripts/inventory_write_paths.py` | target not found | UNKNOWN | 0 |

A pre-edit `gitnexus_detect_changes` over current planning/artifact changes reported LOW risk with affected_processes=0, but scanner edit blast radius remains UNKNOWN because symbol/file lookup failed.

## Blast radius statement

The pre-edit scanner blast radius is **UNKNOWN**. M174 proceeds only with exact source-path classifier additions, a preserved exception test, focused fallback tests, regenerated inventory, and final `gitnexus_detect_changes`.

## Code surface

Scanner category logic is centralized in `_classify(source_path, operation, target, mode)`. M174 must not change AST traversal, record schema, markdown rendering, or JSON payload shape.

## Focused test plan

Positive test:

1. `src/research_graph/infrastructure/repair/chunking_benchmark.py` or non-index `chunk_baseline_measurement.py` returns `repair-benchmark-output`.

Preserved exception test:

1. `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` + `index_path` remains `caller-owned-index`.

Fallback test:

1. Unapproved repair-like path with `diagnostics_path` or `summary_path` does not become `repair-benchmark-output`.
2. Unreviewed state/index/catalog paths remain `shared-state`.

## Edit constraints

- Add exact source path checks only.
- Test `index_path` preservation before assigning `repair-benchmark-output`.
- Do not classify by broad target words.
- Do not classify all repair package writes.
- Keep total record count stable unless scanner traversal changes are explicitly planned, which M174 does not plan.
