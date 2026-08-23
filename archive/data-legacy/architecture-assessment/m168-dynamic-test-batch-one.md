# M168 Dynamic Test Batch One

## Verdict

**S08 status: PASS.**

M168 migrated two of the three remaining dynamic script import tests to normal `scripts.*` imports and moved them into `strict_script_wrapper` coverage.

## Files migrated

### `tests/test_m060d_s01.py`

Changed:

- replaced dynamic `importlib.util.spec_from_file_location(...)` loader for `scripts/check_project_trajectory.py` with `from scripts import check_project_trajectory`;
- removed dynamic loader helper;
- aligned stale project trajectory assertions to current advisory report semantics without rewriting historical artifacts.

Focused result:

```text
uv run pytest tests/test_m060d_s01.py -q
10 passed
```

### `tests/test_m062_s03.py`

Changed:

- replaced dynamic `importlib.util.spec_from_file_location(...)` loader for `scripts/test_fd_contract.py` with `from scripts import test_fd_contract`;
- removed dynamic loader helper;
- updated stale embedder source path from removed `src/arxiv_archive/embedder.py` to current `src/research_graph/infrastructure/retrieval/embedder.py`.

Focused result:

```text
uv run pytest tests/test_m062_s03.py -q
8 passed
```

## Allowlist change

Before S08:

```text
allowlisted_dynamic_script_import=3
allowlisted_legacy_mixed=3
strict_script_wrapper=54
```

After S08:

```text
allowlisted_dynamic_script_import=1
allowlisted_legacy_mixed=1
strict_script_wrapper=56
violations=0
```

Remaining dynamic file:

- `tests/test_m061_s03.py`

## Verification

```text
uv run pytest tests/test_m060d_s01.py tests/test_m062_s03.py -q
18 passed

uv run python scripts/verify_test_architecture.py --json
status=passed
allowlisted_dynamic_script_import=1
allowlisted_legacy_mixed=1
strict_script_wrapper=56
violations=0

uv run ruff check tests/test_m060d_s01.py tests/test_m062_s03.py
All checks passed
```

## Notes

The focused tests exposed stale assertions unrelated to dynamic import mechanics. Repairs were limited to current authoritative state:

- project trajectory check now accepts current advisory `drift_risk` semantics and requires drift flags when present;
- embedder path now points to the current `research_graph` package location.
