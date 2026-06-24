# M151 M060 Batch Script Wrapper Ratchet

Schema: `daily-archive-m151-m060-batch-ratchet.v1`

Promoted:
- `tests/test_m060c_s01.py`
- `tests/test_m060c_s02.py`
- `tests/test_m060g_s01.py`

Excluded:
- `tests/test_m060g_s02.py`: baseline focused pytest timed out at 300 seconds.

Classification: `strict_script_wrapper`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 24 | 21 | -3 |
| `legacy_mixed` | 38 | 35 | -3 |
| `strict_script_wrapper` | 34 | 37 | +3 |
| `strict_infrastructure` | 6 | 6 | +0 |

## Rationale

Three selected M060 tests were baseline-green and now import their scripts through normal repo-root `scripts` imports instead of dynamic importlib loading.

## Verification

- Focused pytest: `21 passed, 2 skipped`.
- Ruff: passed.
- Pyrefly: `0 errors`.
