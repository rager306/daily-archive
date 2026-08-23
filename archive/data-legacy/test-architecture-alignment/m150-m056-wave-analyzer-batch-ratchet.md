# M150 M056 Wave Analyzer Batch Script Wrapper Ratchet

Schema: `daily-archive-m150-m056-wave-analyzer-batch-ratchet.v1`

Promoted:
- `tests/test_m056_wave_4.py`
- `tests/test_m056_wave_5.py`
- `tests/test_m056_wave_6.py`

Classification: `strict_script_wrapper`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 27 | 24 | -3 |
| `legacy_mixed` | 41 | 38 | -3 |
| `strict_script_wrapper` | 31 | 34 | +3 |
| `strict_infrastructure` | 6 | 6 | +0 |

## Rationale

Three M056 wave analyzer tests were baseline-green and now import their analyzer scripts through normal repo-root `scripts` imports instead of dynamic importlib loading.

## Verification

- Focused pytest: `18 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
