# M137 Dynamic Ratchet Candidate

Schema: `daily-archive-m137-dynamic-ratchet-candidate.v1`

## Selected file

- `tests/test_article_preprocessing_replay_contract.py`
- Current bucket: `legacy-mixed`
- Baseline pytest: `6 passed`.
- GitNexus blast radius: LOW / max LOW.

## Intended count delta

| Metric | Before | Delta | Expected after |
|---|---:|---:|---:|
| `dynamic_script_import` | 49 | -1 | 48 |
| `legacy_mixed` | 63 | -1 | 62 |
| `strict_script_wrapper` | 10 | +1 | 11 |
| `unknown` | 77 | +0 | 77 |
