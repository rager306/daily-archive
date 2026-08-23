# M136 Dynamic Ratchet Candidate

Schema: `daily-archive-m136-dynamic-ratchet-candidate.v1`

## Selected file

- `tests/test_article_baseline_recovery_replay.py`
- Current bucket: `legacy-mixed`
- Baseline pytest: `11 passed`.
- GitNexus blast radius: LOW / max LOW.

## Intended count delta

| Metric | Before | Delta | Expected after |
|---|---:|---:|---:|
| `dynamic_script_import` | 50 | -1 | 49 |
| `legacy_mixed` | 64 | -1 | 63 |
| `strict_script_wrapper` | 9 | +1 | 10 |
| `unknown` | 77 | +0 | 77 |
