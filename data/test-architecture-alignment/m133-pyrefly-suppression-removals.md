# M133 Pyrefly Suppression Removals

## Removed

| Path | Target | Reason |
|---|---|---|
| `tests/test_replay_m028_smoke_closeout.py` | `scripts.replay_m028_smoke_closeout` | normal scripts import resolved by pyrefly search-path |
| `tests/test_m036_real_corpus_no_write_smoke.py` | `scripts.run_m036_real_corpus_no_write_smoke` | normal scripts import resolved by pyrefly search-path |
| `tests/test_m036_real_corpus_smoke_audit.py` | `scripts.audit_m036_real_corpus_smoke` | normal scripts import resolved by pyrefly search-path |

## Verification

- Pyrefly: `0 errors` for edited files.
- Ruff: passed for edited files.
- Pytest: `12 passed` for edited files.

## Deferred

Legacy script path shims and optional dependency suppressions remain categorized but unmodified.
