# M138 Dynamic Ratchet

Schema: `daily-archive-m138-dynamic-ratchet.v1`

Promoted: `tests/test_bounded_chunk_repair.py`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 48 | 47 | -1 |
| `legacy_mixed` | 62 | 61 | -1 |
| `strict_script_wrapper` | 11 | 12 | +1 |

## Rationale

Baseline-green bounded chunk repair test migrated from importlib script loading to normal scripts imports.

## Verification

- Focused pytest: `17 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
