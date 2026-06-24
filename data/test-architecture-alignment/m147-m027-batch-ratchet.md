# M147 M027 Batch Script Wrapper Ratchet

Schema: `daily-archive-m147-m027-batch-ratchet.v1`

Promoted:
- `tests/test_m027_end_to_end_mixed_replay.py`
- `tests/test_m027_pipeline_readiness_synthesis.py`
- `tests/test_m027_provenance_and_riskratchet_gate.py`
- `tests/test_m027_requirement_scope_reconciliation.py`

Classification: `strict_script_wrapper`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 39 | 35 | -4 |
| `legacy_mixed` | 53 | 49 | -4 |
| `strict_script_wrapper` | 19 | 23 | +4 |
| `strict_infrastructure` | 6 | 6 | +0 |

## Rationale

Four M027 cohort tests were baseline-green and now import their scripts under test through normal repo-root `scripts` imports instead of dynamic importlib loading.

## Verification

- Focused pytest: `85 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
