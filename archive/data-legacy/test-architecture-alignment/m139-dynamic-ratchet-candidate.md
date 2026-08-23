# M139 Dynamic Ratchet Candidate

Schema: `daily-archive-m139-dynamic-ratchet-candidate.v1`

## Selected file

- `tests/test_codebase_memory_governance.py`
- Current bucket: `legacy-mixed`
- Baseline pytest: `10 passed`.
- GitNexus blast radius: LOW.

## Intended count delta

| Metric | Before | Delta | Expected after |
|---|---:|---:|---:|
| `dynamic_script_import` | 47 | -1 | 46 |
| `legacy_mixed` | 61 | -1 | 60 |
| `strict_script_wrapper` | 12 | +1 | 13 |
| `unknown` | 77 | +0 | 77 |
