# M171 Inventory Category Recon

## Verdict

**Richer write-path inventory categories are feasible with narrow, path-specific rules.**

The current scanner conservatively marks any path containing `queue`, `state`, `index`, or `catalog` as `shared-state` before checking caller-owned or run-scoped ownership. That preserved risk visibility, but M170 reviewed four records whose ownership is more precise than generic shared state.

## Current baseline

Evidence: `data/architecture-assessment/m171-write-path-inventory-baseline.json` and `gsd_exec[b0d5a696-2a57-4d51-9be5-207238dc3b2b]`.

```text
total_records=340
unknown=0
shared-state=4
```

Current shared-state records:

| Record | Current target | M170 disposition | Proposed category |
|---|---|---|---|
| `src/research_graph/application/validation/batch_state.py:252` | `output_path` | workflow-owned replacement | `run-owned-state` |
| `src/research_graph/infrastructure/corpus/ingestion/catalog_adapters.py:540` | `summary_path` | legacy one-shot evidence summary | `legacy-evidence-regeneration` |
| `src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py:935` | `report_path` | human-readable evidence report | `legacy-evidence-regeneration` |
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py:183` | `index_path` | caller-owned paired review output | `caller-owned-index` |

## Proposed category rules

Rules must be exact enough not to hide new shared-state risk:

1. `run-owned-state`: only `src/research_graph/application/validation/batch_state.py` target `output_path`.
2. `legacy-evidence-regeneration`: only reviewed legacy summary/report writers with targets `summary_path` or `report_path` in catalog evidence paths.
3. `caller-owned-index`: only `chunk_baseline_measurement.py` target `index_path`, paired with explicit caller-provided review output/index arguments.

## Safety rules

- Keep generic `state`, `index`, and `catalog` detection for all other records.
- Do not add broad target-token exceptions for `state_path`, `index_path`, or `report_path` globally.
- Preserve `unknown=0`.
- Tests must assert the four reviewed records move to precise categories and no broad category hides synthetic unsafe state/index records.

## Edit targets for S09/S10

- `scripts/inventory_write_paths.py::_classify(...)`.
- Tests likely in `tests/test_pipeline_script_inventory.py` or a new focused scanner test if existing coverage is not suitable.

## Expected post-change counts

Expected category changes:

```text
shared-state: 4 -> 0
run-owned-state: 1
legacy-evidence-regeneration: 2
caller-owned-index: 1
unknown: 0
```

If new records appear during implementation, counts may differ, but the four reviewed records must receive precise categories and unknown must remain zero.
