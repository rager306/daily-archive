# M152 Connectivity Sidecar Batch Script Wrapper Ratchet

Schema: `daily-archive-m152-connectivity-sidecar-batch-ratchet.v1`

Promoted:
- `tests/test_m041_mixed_connectivity_batch.py`
- `tests/test_m042_connectivity_groups.py`
- `tests/test_m042_linked_metadata_repair.py`
- `tests/test_m043_sidecar_packets.py`
- `tests/test_m043_sidecar_runtime_readiness.py`

Classification: `strict_script_wrapper`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 21 | 16 | -5 |
| `legacy_mixed` | 35 | 30 | -5 |
| `strict_script_wrapper` | 37 | 42 | +5 |
| `strict_infrastructure` | 6 | 6 | +0 |

## Rationale

Five M041-M043 connectivity-sidecar tests were baseline-green and now import their scripts through normal repo-root `scripts` imports instead of dynamic importlib loading.

## Compatibility notes

- tests/test_m041_mixed_connectivity_batch.py retains scripts-directory path setup because the script under test imports a sibling script by bare module name.

## Verification

- Focused pytest: `20 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
