# M136 Dynamic Ratchet

Schema: `daily-archive-m136-dynamic-ratchet.v1`

Promoted: `tests/test_article_baseline_recovery_replay.py`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 50 | 49 | -1 |
| `legacy_mixed` | 64 | 63 | -1 |
| `strict_script_wrapper` | 9 | 10 | +1 |

## Rationale

Baseline-green M025 recovery replay test migrated from importlib script loading to normal scripts imports.

## Verification

- Focused pytest: `11 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
