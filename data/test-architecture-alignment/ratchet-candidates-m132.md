# M132 Ratchet Candidates

Schema: `daily-archive-test-ratchet-candidates.v1`

## Selected batch

| Path | Current bucket | Target bucket | Strategy |
|---|---|---|---|
| `tests/test_m061_s02.py` | `legacy-mixed` | `script-wrapper` | Replace dynamic loading of `scripts/m061_full_5_anchors.py` with a normal `from scripts import m061_full_5_anchors` import after M131 fixture repair. |

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 52 | 51 | -1 |
| `legacy_mixed` | 66 | 65 | -1 |
| `strict_script_wrapper` | 6 | 7 | +1 |

## Suppression cleanup

M132 also configures pyrefly `search-path = ["."]`, so prior normal `from scripts import ...` tests no longer need local `pyrefly: ignore [missing-import]` comments.
