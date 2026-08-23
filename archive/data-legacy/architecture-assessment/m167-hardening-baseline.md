# M167 Hardening Baseline

## Scope

M167 includes the next three follow-ups together:

1. Classify production write paths.
2. Review and probe `UniversalKBQueue` concurrency behavior.
3. Reduce dynamic/legacy test architecture allowlists where safe.

## Impact

GitNexus impact was run before queue edits on exact method UIDs:

| Target | Risk | Affected processes |
|---|---|---:|
| `UniversalKBQueue.claim` | LOW | 0 |
| `UniversalKBQueue.complete` | LOW | 0 |
| `UniversalKBQueue.reclaim_expired_leases` | LOW | 0 |

GitNexus context for `UniversalKBQueue` shows package imports from `substrate_rehearsal.py` and `rehearsal.py`; method-level upstream impact for key concurrency methods is currently zero.

## Guard baseline

Evidence: `.gsd/exec/535ea7fe-3f9a-44df-ac01-4768357a69c1.stdout`

```text
onion guard: status=clear, violation_count=0, allowed_violation_count=0
test architecture: status=passed, violations=0
allowlisted_dynamic_script_import=3
allowlisted_legacy_mixed=18
strict_application=6
strict_infrastructure=6
strict_script_wrapper=54
total_test_files=269
```

## Write paths baseline

Evidence: `.gsd/exec/9c9a26c6-8127-45e4-a5b1-bcc4eba1f386.stdout`

Rough pre-tool scan found:

| Root | Files with write-like matches | Write-like matches |
|---|---:|---:|
| `src/research_graph` | 43 | 122 |
| `scripts` | 167 | 445 |

This scan is intentionally rough and includes false positives from string `.replace()` calls. S02 will create a deterministic inventory tool with better categories.

## UniversalKBQueue baseline

Evidence: `.gsd/exec/1030900c-659d-4bec-9feb-7b4cadb8158a.stdout`

Existing tests already cover some lease behavior:

- exclusive claim sets lease fields,
- heartbeat extends matching lease and rejects wrong owner,
- complete clears lease,
- second worker cannot claim while a lease is active,
- expired leases reclaim to ready until attempts exhaust.

Current S04/S05 focus: review whether these tests cover actual multi-connection contention or only single-connection sequential behavior, then add a minimal probe if feasible.

## Allowlist baseline

Current test architecture guard reports:

```text
allowlisted_dynamic_script_import=3
allowlisted_legacy_mixed=18
```

M165 identified known dynamic candidates:

- `tests/test_m060d_s01.py`
- `tests/test_m061_s03.py`
- `tests/test_m062_s03.py`

S06/S07 will inspect the allowlist and candidate tests before changing anything.

## Feasibility verdict

All three requested items are feasible together in one milestone if scoped as:

- write-path classification, not full migration of every writer,
- queue concurrency review plus focused contract probe, not full queue redesign,
- safe allowlist reduction only, not forced historical rewrites.
