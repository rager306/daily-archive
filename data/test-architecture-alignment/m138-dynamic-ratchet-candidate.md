# M138 Dynamic Ratchet Candidate

Schema: `daily-archive-m138-dynamic-ratchet-candidate.v1`

## Selected file

- `tests/test_bounded_chunk_repair.py`
- Current bucket: `legacy-mixed`
- Baseline pytest: `17 passed`.
- GitNexus blast radius: LOW for target file symbols.

## Intended count delta

| Metric | Before | Delta | Expected after |
|---|---:|---:|---:|
| `dynamic_script_import` | 48 | -1 | 47 |
| `legacy_mixed` | 62 | -1 | 61 |
| `strict_script_wrapper` | 11 | +1 | 12 |
| `unknown` | 77 | +0 | 77 |
