# M175 Focused Test Plan

## Verdict

**Test plan status: READY.** Tests must prove exact categories, no-move fallbacks, and delta report rendering before final inventory regeneration.

## Classification tests

| Rule | Test expectation |
|---|---|
| `daily-cli-output` positive | `src/research_graph/cli/__init__.py` + `filepath` returns `daily-cli-output`. |
| `daily-cli-output` daily artifacts | `src/research_graph/cli/__init__.py` + `day_dir / 'papers.json'` returns `daily-cli-output`. |
| CLI temp no-move | `src/research_graph/cli/__init__.py` + `temp_path` remains `temporary`. |
| Generic filepath fallback | Another source file + `filepath` remains `caller-owned`. |
| `validation-batch-output` positive | `src/research_graph/workflows/validation/batch_workflow.py` + `summary_path` returns `validation-batch-output`. |
| Generic summary fallback | Another source file + `summary_path` remains its existing broad category. |
| Existing M174 repair exception | `chunk_baseline_measurement.py` + `index_path` remains `caller-owned-index`. |

## Delta report tests

| Rule | Test expectation |
|---|---|
| Category additions | New category appears with baseline 0, current N, positive delta. |
| Category decreases | Reduced broad category appears with negative delta. |
| Total delta | Report includes baseline total, current total, and delta. |
| Deterministic ordering | Rows are sorted by category name for stable CI artifacts. |

## Command checks after implementation

```text
uv run pytest tests/test_inventory_write_paths.py -q
uv run ruff check scripts/inventory_write_paths.py tests/test_inventory_write_paths.py
```

## Final verification dependencies

- Final inventory must show `daily-cli-output=5`.
- Final inventory must show `validation-batch-output=10`.
- Final inventory must keep `unknown=0` and `shared-state=0`.
- Generated delta must be produced by scanner CLI, not hand-written arithmetic.
