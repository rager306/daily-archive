# M173 Impact Analysis and Test Plan

## Pre-edit GitNexus impact

GitNexus impact was attempted before editing scanner code.

| Target | Result | Risk | Impacted count |
|---|---|---|---:|
| `_classify` | target not found | UNKNOWN | 0 |
| `scripts/inventory_write_paths.py` | target not found | UNKNOWN | 0 |

A pre-edit `gitnexus_detect_changes` over current planning/artifact changes reported LOW risk with affected_processes=0, but scanner edit blast radius remains UNKNOWN because symbol/file lookup failed.

## Blast radius statement

The pre-edit scanner blast radius is **UNKNOWN**. M173 proceeds only with exact source-path classifier additions, focused positive/fallback tests, regenerated inventory, and final `gitnexus_detect_changes`.

## Code surface

Scanner category logic is centralized in `_classify(source_path, operation, target, mode)`. M173 must not change AST traversal, record schema, markdown rendering, or JSON payload shape.

## Focused test plan

Positive tests:

1. `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` returns `parser-replay-output`.
2. `src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py` or `thirty_paper_deviation_scan.py` returns `source-scan-output`.
3. `src/research_graph/infrastructure/graph/r024_networkx_probe.py` returns `graph-probe-output`.

Fallback tests:

1. Unapproved parser-like path with `cache_path` remains `caller-owned` or conservative according to existing rules.
2. Unapproved source-like path with `destination` remains `caller-owned`.
3. Unapproved graph module with `summary_path` remains `caller-owned`.
4. Unreviewed state/index/catalog paths remain `shared-state`.

## Edit constraints

- Add exact source path checks only.
- Do not classify by broad target words.
- Do not reclassify `shared-state` records.
- Keep total record count stable unless scanner traversal changes are explicitly planned, which M173 does not plan.
