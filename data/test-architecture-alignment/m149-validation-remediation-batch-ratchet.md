# M149 Validation Remediation Batch Script Wrapper Ratchet

Schema: `daily-archive-m149-validation-remediation-batch-ratchet.v1`

Promoted:
- `tests/test_m029_post_validation_remediation.py`
- `tests/test_m029_validation_remediation.py`
- `tests/test_m031_validation_remediation.py`

Classification: `strict_script_wrapper`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 30 | 27 | -3 |
| `legacy_mixed` | 44 | 41 | -3 |
| `strict_script_wrapper` | 28 | 31 | +3 |
| `strict_infrastructure` | 6 | 6 | +0 |

## Rationale

Three validation remediation verifier tests were baseline-green and now import their verifier scripts through normal repo-root `scripts` imports instead of dynamic importlib loading.

## Verification

- Focused pytest: `43 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
