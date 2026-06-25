# M167 Write Path Queue and Test Allowlist Hardening Closeout

## Verdict

**M167 status: PASS.**

M167 included items 1, 2, and 3 together and completed each within the planned scope.

## Item 1: production write path classification

Status: **closed for classification scope**.

Delivered:

- `scripts/inventory_write_paths.py`
- `data/architecture-assessment/m167-write-path-inventory.json`
- `data/architecture-assessment/m167-write-path-inventory.md`
- `data/architecture-assessment/m167-write-path-classification.md`

Inventory summary:

```text
total_records=344
script-only=263
run-scoped=41
unknown=26
append-log=7
shared-state=6
database=1
```

Main P1 follow-up from classification: canonical catalog article/index writes in `catalog_ingest.py` need atomic/single-writer review.

## Item 2: UniversalKBQueue concurrency review and probe

Status: **closed for focused review and contract-probe scope**.

Delivered:

- `data/architecture-assessment/m167-queue-concurrency-recon.md`
- `data/architecture-assessment/m167-queue-concurrency-result.md`
- `UniversalKBQueue.claim()` rowcount hardening
- `tests/test_universal_kb_queue.py::test_multi_connection_claim_allows_only_one_worker`

Change:

- `claim()` now checks whether its guarded update won the state transition.
- If `cursor.rowcount != 1`, it returns `None` without inserting a false claim event.

Verification:

```text
uv run pytest tests/test_universal_kb_queue.py -q
23 passed
```

Remaining limitation: this is a focused multi-connection contention contract, not a full multiprocess stress suite.

## Item 3: dynamic and legacy allowlist reduction

Status: **closed for safe reduction scope**.

Delivered:

- `strict_workflows` bucket in `scripts/verify_test_architecture.py`
- updated `data/test-architecture-alignment/test-architecture-allowlist.json`
- updated `tests/test_test_architecture_guardrail.py`
- `data/architecture-assessment/m167-test-allowlist-recon.md`
- `data/architecture-assessment/m167-test-allowlist-result.md`

Before:

```text
allowlisted_dynamic_script_import=3
allowlisted_legacy_mixed=18
strict_workflows=0
```

After:

```text
allowlisted_dynamic_script_import=3
allowlisted_legacy_mixed=3
strict_workflows=15
violations=0
```

The remaining three dynamic files are true dynamic loader debt and should be handled by future per-file refactors.

## Verification

| Check | Result |
|---|---|
| Write-path inventory rerun | PASS: 344 records, stable summary |
| Queue tests | PASS: 23 passed |
| Test architecture guard | PASS: legacy=3, dynamic=3, strict_workflows=15, violations=0 |
| Onion guard | PASS: violation_count=0, allowed_violation_count=0 |
| Scoped ruff | PASS |
| Pyrefly | PASS: 0 errors |
| Pre-commit | PASS |
| GitNexus detect changes | LOW risk, affected_processes=0 |

## Remaining backlog

1. Review canonical catalog article/index writes for atomic replacement or explicit single-writer contract.
2. Add longer multiprocess UniversalKBQueue stress tests before high-concurrency queue activation.
3. Refactor the remaining dynamic script import files:
   - `tests/test_m060d_s01.py`
   - `tests/test_m061_s03.py`
   - `tests/test_m062_s03.py`
4. Reduce `unknown` write-path records through better scanner heuristics or local ownership annotations.

## Files changed

- `scripts/inventory_write_paths.py`
- `scripts/verify_test_architecture.py`
- `src/research_graph/workflows/universal_kb/queue.py`
- `tests/test_universal_kb_queue.py`
- `tests/test_test_architecture_guardrail.py`
- `data/test-architecture-alignment/test-architecture-allowlist.json`
- `data/architecture-assessment/m167-*.md`
- `data/architecture-assessment/m167-write-path-inventory.json`

## Closeout conclusion

M167 provides the missing write-path classification map, hardens the primary queue claim race shape with an executable multi-connection contract, and ratchets test architecture allowlist debt down from 18 legacy-mixed entries to 3.
