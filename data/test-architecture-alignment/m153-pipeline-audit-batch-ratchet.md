# M153 Pipeline Audit Batch Script Wrapper Ratchet

Schema: `daily-archive-m153-pipeline-audit-batch-ratchet.v1`

Promoted:
- `tests/test_pipeline_script_audit.py`
- `tests/test_pipeline_script_wrapper_contracts.py`

Classification: `strict_script_wrapper`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 16 | 14 | -2 |
| `legacy_mixed` | 30 | 28 | -2 |
| `strict_script_wrapper` | 42 | 44 | +2 |
| `strict_infrastructure` | 6 | 6 | +0 |

## Rationale

Two pipeline audit tests were baseline-green and now import `scripts.audit_pipeline_scripts` normally instead of dynamic importlib loading.

## Exclusions

The batch stayed to two files because nearby candidates were baseline-red, timed out, or had a different bucket shape.

## Verification

- Focused pytest: `15 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
