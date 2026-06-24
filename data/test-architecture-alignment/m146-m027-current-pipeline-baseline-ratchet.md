# M146 M027 Current Pipeline Baseline Script Wrapper Ratchet

Schema: `daily-archive-m146-m027-current-pipeline-baseline-ratchet.v1`

Promoted: `tests/test_m027_current_pipeline_baseline.py`
Classification: `strict_script_wrapper`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 40 | 39 | -1 |
| `legacy_mixed` | 54 | 53 | -1 |
| `strict_script_wrapper` | 18 | 19 | +1 |
| `strict_infrastructure` | 6 | 6 | +0 |

## Rationale

The M027 current pipeline baseline test was baseline-green and now imports its replay and verifier scripts through normal repo-root `scripts` imports instead of dynamic importlib loading.

## Verification

- Focused pytest: `13 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
