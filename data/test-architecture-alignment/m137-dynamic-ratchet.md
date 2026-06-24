# M137 Dynamic Ratchet

Schema: `daily-archive-m137-dynamic-ratchet.v1`

Promoted: `tests/test_article_preprocessing_replay_contract.py`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 49 | 48 | -1 |
| `legacy_mixed` | 63 | 62 | -1 |
| `strict_script_wrapper` | 10 | 11 | +1 |

## Rationale

Baseline-green M025 final preprocessing replay test migrated from importlib script loading to normal scripts imports.

## Verification

- Focused pytest: `6 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
