# M168 Dynamic Test Final

## Verdict

**Backlog item 3 status: PARTIAL CLOSED.**

M168 safely reduced dynamic script import debt from 3 files to 1 file. The final remaining file, `tests/test_m061_s03.py`, is blocked by stale-red historical artifact assertions, not by import mechanics.

## Completed reductions

Moved to normal `scripts.*` imports and `strict_script_wrapper` coverage:

- `tests/test_m060d_s01.py`
- `tests/test_m062_s03.py`

Current guard state:

```text
allowlisted_dynamic_script_import=1
allowlisted_legacy_mixed=1
strict_script_wrapper=56
violations=0
```

## Remaining blocker

`tests/test_m061_s03.py` can import `scripts.m061_synthesis` normally, but the focused test is currently baseline-red after import migration because historical artifact assertions no longer match current files:

```text
uv run pytest tests/test_m061_s03.py -q
2 failed, 5 passed
```

Observed failures:

- protected hash for `artifacts/m061-2hop/s01-decision.md` differs from the frozen expected hash;
- `m061_synthesis.collect_summary(...)` recomputes aggregate throughput/pacing values that differ from the frozen `m061-summary.json` artifact.

This is not safe to repair inside an import-style ratchet without deciding which historical M061 artifact is authoritative. Per M168 constraints, S09 does not rewrite historical artifacts or loosen protected hash checks just to remove an allowlist entry.

## Decision

Keep `tests/test_m061_s03.py` in:

- `dynamic_script_import`
- `legacy_mixed`

until a dedicated historical M061 artifact reconciliation slice decides whether to:

1. update frozen hashes/summaries to current authoritative artifacts;
2. restore historical files to the frozen hashes;
3. split the import-style check from the historical artifact regression contract.

## Verification

```text
uv run pytest tests/test_m060d_s01.py tests/test_m062_s03.py -q
18 passed

uv run python scripts/verify_test_architecture.py --json
status=passed
allowlisted_dynamic_script_import=1
allowlisted_legacy_mixed=1
strict_script_wrapper=56
violations=0

uv run ruff check tests/test_m060d_s01.py tests/test_m062_s03.py
All checks passed
```

## Final dynamic debt after M168

- Closed: 2/3 files.
- Remaining: 1/3 file, with documented historical artifact blocker.
