# M142 Boundary Replay Script Wrapper Ratchet

Schema: `daily-archive-m142-boundary-replay-ratchet.v1`

Promoted: `tests/test_m025_boundary_replay_completion.py`
Classification: `strict_script_wrapper`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 44 | 43 | -1 |
| `legacy_mixed` | 58 | 57 | -1 |
| `strict_script_wrapper` | 14 | 15 | +1 |
| `strict_infrastructure` | 6 | 6 | +0 |

## Rationale

The M025 boundary replay test was baseline-green and now imports its script under test through a normal repo-root `scripts` import instead of dynamic importlib loading.

## Verification

- Focused pytest: `7 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
