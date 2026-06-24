# M143 Evidence Replay Script Wrapper Ratchet

Schema: `daily-archive-m143-evidence-replay-ratchet.v1`

Promoted: `tests/test_m025_evidence_replay.py`
Classification: `strict_script_wrapper`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 43 | 42 | -1 |
| `legacy_mixed` | 57 | 56 | -1 |
| `strict_script_wrapper` | 15 | 16 | +1 |
| `strict_infrastructure` | 6 | 6 | +0 |

## Rationale

The M025 evidence replay test was baseline-green and now imports its script under test through a normal repo-root `scripts` import instead of dynamic importlib loading.

## Verification

- Focused pytest: `5 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
