# M168 Dynamic Test Refactor Recon

## Baseline

Current allowlist counts:

```text
allowlisted_dynamic_script_import=3
allowlisted_legacy_mixed=3
strict_workflows=15
violations=0
```

Remaining dynamic files:

- `tests/test_m060d_s01.py`
- `tests/test_m061_s03.py`
- `tests/test_m062_s03.py`

## Normal import probe

Command:

```text
uv run python - <<'PY'
from scripts import check_project_trajectory, m061_synthesis, test_fd_contract
print(check_project_trajectory.__name__)
print(m061_synthesis.__name__)
print(test_fd_contract.__name__)
PY
```

Result:

```text
scripts.check_project_trajectory
scripts.m061_synthesis
scripts.test_fd_contract
```

Evidence: `gsd_exec[dbc614e1-e7f1-45cc-8812-2dd2eadd34cb]`.

## Per-file disposition

| File | Dynamic target | Current pattern | Proposed migration | Risk |
|---|---|---|---|---|
| `tests/test_m060d_s01.py` | `scripts/check_project_trajectory.py` | `importlib.util.spec_from_file_location` in `_load_trajectory_module()` | Replace loader with `from scripts import check_project_trajectory`; remove `importlib.util`/`sys`; keep subprocess tests for process-boundary scripts | low |
| `tests/test_m061_s03.py` | `scripts/m061_synthesis.py` | `importlib.util.spec_from_file_location` in `load_synthesis_module()` | Replace loader with `from scripts import m061_synthesis`; remove `importlib.util`/`sys`; keep hash/artifact assertions unchanged | medium because M061 artifacts are historical but assertions are not rewritten |
| `tests/test_m062_s03.py` | `scripts/test_fd_contract.py` | `importlib.util.spec_from_file_location` in `_load_contract_module()` | Replace loader with `from scripts import test_fd_contract`; remove `importlib.util`/`sys`; keep subprocess down-service contract unchanged | low |

## Batch plan

S08:

- migrate `tests/test_m060d_s01.py`;
- migrate `tests/test_m062_s03.py`;
- run both focused tests and the guard.

S09:

- migrate `tests/test_m061_s03.py` separately because it protects historical M061 artifacts and hashes;
- run focused M061 test and guard;
- if green, remove all three files from `dynamic_script_import` and `legacy_mixed`, and add them to `strict_script_wrapper`.

Expected final allowlist if all migrations pass:

```text
allowlisted_dynamic_script_import=0
allowlisted_legacy_mixed=0
strict_script_wrapper += 3
violations=0
```

## Guard expectation

After normal imports, these tests should classify as script-wrapper/acceptance style tests because they exercise scripts as process-boundary wrappers. They should move to `strict_script_wrapper`, not `strict_workflows`.

## Blockers

No blocker found in S07. All three dynamic targets are importable through normal `scripts.*` imports in the current environment.
