# M139 Dynamic Ratchet

Schema: `daily-archive-m139-dynamic-ratchet.v1`

Promoted: `tests/test_codebase_memory_governance.py`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 47 | 46 | -1 |
| `legacy_mixed` | 61 | 60 | -1 |
| `strict_script_wrapper` | 12 | 13 | +1 |

## Rationale

Baseline-green codebase memory governance test migrated from importlib script loading to normal scripts import.

## Verification

- Focused pytest: `10 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
