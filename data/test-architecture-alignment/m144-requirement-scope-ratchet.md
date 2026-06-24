# M144 Requirement Scope Script Wrapper Ratchet

Schema: `daily-archive-m144-requirement-scope-ratchet.v1`

Promoted: `tests/test_m025_requirement_scope_reconciliation.py`
Classification: `strict_script_wrapper`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 42 | 41 | -1 |
| `legacy_mixed` | 56 | 55 | -1 |
| `strict_script_wrapper` | 16 | 17 | +1 |
| `strict_infrastructure` | 6 | 6 | +0 |

## Rationale

The M025 requirement scope reconciliation test was baseline-green and now imports its script under test through a normal repo-root `scripts` import instead of dynamic importlib loading.

## Verification

- Focused pytest: `30 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
