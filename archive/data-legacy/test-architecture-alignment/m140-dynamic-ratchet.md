# M140 Dynamic Ratchet

Schema: `daily-archive-m140-dynamic-ratchet.v1`

Skipped: `tests/test_dspy_extraction_boundary.py` baseline-red before migration.
Promoted: `tests/test_m024_validation_evidence_closure.py`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 46 | 45 | -1 |
| `legacy_mixed` | 60 | 59 | -1 |
| `strict_script_wrapper` | 13 | 14 | +1 |

## Rationale

Baseline-green M024 validation evidence closure test migrated from importlib script loading to normal scripts import after DSPy candidate was skipped for baseline failure.

## Verification

- Focused pytest: `14 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
