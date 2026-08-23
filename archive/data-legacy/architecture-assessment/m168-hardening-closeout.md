# M168 Catalog Queue Dynamic Tests and Write Path Hardening Closeout

## Verdict

**M168 status: PASS with one documented partial item.**

M168 attempted all four requested backlog items together. Items 1, 2, and 4 are closed for their planned scopes. Item 3 is partially closed: dynamic/legacy test allowlists were reduced from 3 to 1, with the remaining M061 test blocked by historical artifact drift.

## Item 1: canonical catalog write safety

Status: **closed for canonical JSON atomicity scope**.

Delivered:

- `_atomic_write_text(...)` in `src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py`;
- `write_article_record(...)` routes through atomic replacement;
- `update_index_if_exists(...)` routes through atomic replacement;
- focused tests for helper behavior, article write delegation, index write delegation, and filesystem adapter delegation;
- artifacts:
  - `data/architecture-assessment/m168-catalog-write-recon.md`
  - `data/architecture-assessment/m168-catalog-write-result.md`

Verification:

```text
uv run pytest tests/test_catalog_ingest.py tests/test_catalog_ingest_filesystem_adapter.py -q
35 passed
```

Residual limit: index merge semantics still rely on process-boundary single-writer behavior; M168 prevents partial/truncated JSON replacement, not multi-writer index merging.

## Item 2: UniversalKBQueue concurrency stress

Status: **closed for bounded stress scope**.

Delivered:

- bounded multi-worker stress test in `tests/test_universal_kb_queue.py`;
- 24 jobs, 6 worker threads, separate SQLite connections;
- asserts unique claims/completions, final succeeded states, and one claim/complete event per job;
- artifacts:
  - `data/architecture-assessment/m168-queue-stress-recon.md`
  - `data/architecture-assessment/m168-queue-stress-result.md`

Verification:

```text
uv run pytest tests/test_universal_kb_queue.py -q
24 passed

stress target repeated 5/5
```

Residual limit: this is a fast thread-level separate-connection proof, not a full multiprocess soak.

## Item 3: dynamic and legacy test allowlist reduction

Status: **partially closed with blocker evidence**.

Delivered:

- `tests/test_m060d_s01.py` migrated from dynamic loader to `from scripts import check_project_trajectory`;
- `tests/test_m062_s03.py` migrated from dynamic loader to `from scripts import test_fd_contract`;
- allowlist updated: those two files moved from `dynamic_script_import`/`legacy_mixed` to `strict_script_wrapper`;
- artifacts:
  - `data/architecture-assessment/m168-dynamic-test-recon.md`
  - `data/architecture-assessment/m168-dynamic-test-batch-one.md`
  - `data/architecture-assessment/m168-dynamic-test-final.md`

Before:

```text
allowlisted_dynamic_script_import=3
allowlisted_legacy_mixed=3
strict_script_wrapper=54
```

After:

```text
allowlisted_dynamic_script_import=1
allowlisted_legacy_mixed=1
strict_script_wrapper=56
violations=0
```

Remaining blocker:

- `tests/test_m061_s03.py`

Normal import of `scripts.m061_synthesis` works, but the focused test is baseline-red on historical artifact hash and recomputed summary aggregate differences. M168 did not rewrite historical artifacts or weaken protected hash checks to force count zero.

## Item 4: unknown write-path reduction

Status: **closed for safe scanner reduction scope**.

Delivered:

- `caller-owned` scanner category for caller-provided or adapter-owned output paths;
- `temporary` scanner category for same-directory temp writes;
- final inventory artifacts:
  - `data/architecture-assessment/m168-write-path-inventory.json`
  - `data/architecture-assessment/m168-write-path-inventory.md`
- artifact:
  - `data/architecture-assessment/m168-write-path-unknown-reduction.md`

Before:

```text
unknown=26
```

After:

```text
unknown=3
caller-owned=38
temporary=1
```

Remaining unknowns:

- CLI per-paper `paper.json` / `scored.json` writes under stable directory;
- fetcher `pdf_path` write whose ownership depends on caller context.

## Integrated verification

Artifact: `data/architecture-assessment/m168-integrated-verification.md`

| Check | Result |
|---|---|
| Focused pytest suite | PASS: 83 passed |
| Final write-path inventory | PASS: unknown=3 |
| Test architecture guard | PASS: dynamic=1, legacy=1, violations=0 |
| Onion guard | PASS: violation_count=0, allowed_violation_count=0 |
| Scoped ruff | PASS |
| Pyrefly | PASS: 0 errors |
| Pre-commit | PASS |
| GitNexus detect_changes | PASS: LOW risk, affected_processes=0 |
| Scope hygiene | PASS |

## Key files changed

- `src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py`
- `scripts/inventory_write_paths.py`
- `tests/test_catalog_ingest.py`
- `tests/test_catalog_ingest_filesystem_adapter.py`
- `tests/test_universal_kb_queue.py`
- `tests/test_m060d_s01.py`
- `tests/test_m062_s03.py`
- `data/test-architecture-alignment/test-architecture-allowlist.json`
- `data/test-architecture-alignment/test-architecture-guardrail.json`
- `data/test-architecture-alignment/test-architecture-guardrail.md`
- `data/architecture-assessment/m168-*.md`
- `data/architecture-assessment/m168-write-path-inventory.json`

## Follow-up backlog

1. Dedicated M061 artifact reconciliation for `tests/test_m061_s03.py`:
   - decide whether frozen hashes/summaries or current artifacts are authoritative;
   - then split or migrate the dynamic import safely.
2. Review remaining 3 unknown write-path records for ownership.
3. Consider atomic/temporary PDF copy for catalog/fetcher paths if future write-safety scope requires it.
4. Add a slower multiprocess UniversalKBQueue soak before high-concurrency activation.

## Conclusion

M168 closed the requested batch as far as safe in one milestone: catalog JSON writes are atomic, queue concurrency proof is stronger, dynamic allowlists are reduced with blocker evidence, and write-path unknowns dropped from 26 to 3 without hiding risky shared-state writes.
