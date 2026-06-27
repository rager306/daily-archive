# M185 Pipeline Audit Extraction Assessment

## Verdict

Proceed with a concrete builder-module extraction.

## GitNexus impact

The following symbols returned LOW, exact impact with no affected processes:

- `build_inventory`
- `_contract`
- `_item`
- `write_inventory`
- `print_summary`

Direct callers are limited to `scripts/audit_pipeline_scripts.py:main` and existing tests.

## Movement scope

Create:

`src/research_graph/application/pipeline_script_audit_inventory.py`

Move the explicit recurring pipeline inventory builder, writer, and summary helpers there. Keep `src/research_graph/application/pipeline_script_inventory.py` as the pure data-contract module. Keep `scripts/audit_pipeline_scripts.py` as a thin CLI wrapper that re-exports the helper names used by tests.

## Constraints

- Preserve generated inventory bytes semantically (`to_dict()` equality).
- Preserve CLI `--repo-root` and `--write` behavior.
- Do not move manifest/cache residuals.
- Do not add new abstractions.
