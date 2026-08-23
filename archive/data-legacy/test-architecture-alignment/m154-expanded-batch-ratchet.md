# M154 Expanded Batch Script Wrapper Ratchet

Schema: `daily-archive-m154-expanded-batch-ratchet.v1`

Promoted out of dynamic import debt:
- `tests/test_m044_live_grobid_candidate_probe.py`
- `tests/test_m056_final_s07.py`
- `tests/test_m061_legacy_delegate.py`
- `tests/test_m063_s01.py`
- `tests/test_m067_s01.py`
- `tests/test_pipeline_architecture_acceptance.py`

Strict script-wrapper promotions:
- `tests/test_m044_live_grobid_candidate_probe.py`
- `tests/test_m056_final_s07.py`
- `tests/test_m061_legacy_delegate.py`
- `tests/test_m063_s01.py`
- `tests/test_m067_s01.py`

Acceptance bucket promotion:
- `tests/test_pipeline_architecture_acceptance.py`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 14 | 8 | -6 |
| `legacy_mixed` | 28 | 23 | -5 |
| `strict_script_wrapper` | 44 | 49 | +5 |
| `strict_infrastructure` | 6 | 6 | +0 |
| `unknown` | 77 | 77 | 0 |

## Rationale

M154 grouped all six currently baseline-green remaining dynamic candidates into one larger GSD milestone while keeping migration slices cohesive.

## Exclusions

The remaining dynamic candidates are baseline-red or timed out and are not included in this ratchet.

## Verification

- Focused pytest: `37 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
