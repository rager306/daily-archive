# M168 Hardening Baseline

## Scope

M168 attempts the next four backlog items together:

1. Canonical catalog article/index write atomicity or explicit single-writer contract.
2. Broader `UniversalKBQueue` concurrency proof beyond M167 focused claim test.
3. Reduction of the remaining dynamic script import test allowlist.
4. Reduction of `unknown` write-path inventory records.

## Baseline counts

### Write-path inventory

Command:

```text
uv run python scripts/inventory_write_paths.py --json /tmp/m168-write-path-baseline.json --markdown /tmp/m168-write-path-baseline.md
```

Result:

```text
total_records=344
script-only=263
run-scoped=41
unknown=26
append-log=7
shared-state=6
database=1
```

Evidence: `gsd_exec[4b0c8f5f-61b6-4f0a-a1a5-5d788e49945a]`.

### Test architecture guard

Command:

```text
uv run python scripts/verify_test_architecture.py --json
```

Result:

```text
status=passed
violations=0
allowlisted_dynamic_script_import=3
allowlisted_legacy_mixed=3
strict_workflows=15
```

Evidence: `gsd_exec[62cd47eb-1042-4622-a6d4-3c84b38b2fe5]`.

Remaining dynamic files from M167:

- `tests/test_m060d_s01.py`
- `tests/test_m061_s03.py`
- `tests/test_m062_s03.py`

### Queue baseline

Command:

```text
uv run pytest tests/test_universal_kb_queue.py -q
```

Result:

```text
23 passed
```

Evidence: `gsd_exec[3c31222d-335f-48b8-84a3-b4be81c71fbe]`.

### Onion guard baseline

Command:

```text
uv run python scripts/verify_onion_layering.py --json
```

Result:

```text
violation_count=0
allowed_violation_count=0
```

Evidence: `gsd_exec[30987078-b0e3-4a86-b080-16ec7ca0e379]`.

## GitNexus impact map

### UniversalKBQueue

Target: `Method:src/research_graph/workflows/universal_kb/queue.py:UniversalKBQueue.claim#2`

Result:

```text
risk=LOW
impactedCount=0
affected_processes=0
```

Target: `Class:src/research_graph/workflows/universal_kb/queue.py:UniversalKBQueue`

Result:

```text
risk=LOW
impactedCount=2
direct imports:
- src/research_graph/workflows/universal_kb/substrate_rehearsal.py
- src/research_graph/workflows/universal_kb/rehearsal.py
affected_processes=0
```

### Catalog write functions

Exact GitNexus targets for the current infrastructure functions were not found:

- `Function:src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py:write_article_record`
- `Function:src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py:update_index_if_exists`

Name-only impact resolved to the older script shim:

- `scripts/m061_ingest_to_canonical_catalog.py:write_article_record`
- `scripts/m061_ingest_to_canonical_catalog.py:update_index_if_exists`

Those name-only results were LOW risk with `affected_processes=0`, but they are stale/not authoritative for the infrastructure module. M168 will treat catalog edits as higher caution and verify with focused tests plus guards.

### Scanner and test guard functions

GitNexus did not find these script-local functions:

- `scripts/inventory_write_paths.py:_classify`
- `scripts/verify_test_architecture.py:verify_inventory`

M168 will rely on focused unit/guard tests and final `gitnexus_detect_changes` for these script-local edits.

## Initial risk decision

Proceed with all four items together, but keep slices thin:

- Catalog write changes get test-first treatment in S03/S04.
- Queue stress is designed before implementation in S05.
- Dynamic test refactors are split into two batches in S08/S09.
- Unknown write-path reduction is kept conservative in S10.

No HIGH or CRITICAL GitNexus risk was reported. The main uncertainty is GitNexus index coverage for current catalog/script-local symbols.
