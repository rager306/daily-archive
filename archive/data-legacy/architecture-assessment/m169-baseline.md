# M169 Baseline

## Verdict

**Baseline status: GREEN.**

All three proposed tracks can start from a passing baseline:

- test architecture guard passes with one remaining dynamic and legacy allowlist entry;
- write-path inventory passes with three remaining unknown records;
- UniversalKBQueue suite passes with the M168 bounded thread-level stress test.

## Evidence

| Check | Result | Evidence |
|---|---|---|
| Test architecture guard | PASS: dynamic=1, legacy=1, violations=0 | `gsd_exec[d4492667-92cd-456e-b962-e868d9a0fa9c]` |
| Write-path inventory | PASS: unknown=3 | `gsd_exec[49fe3e45-6881-46a0-9e0b-3219c2253be6]` |
| UniversalKBQueue baseline | PASS: 24 passed | `gsd_exec[d6f7ed00-bfa9-427f-866d-9d7edc4e066f]` |

## Test architecture baseline

```text
allowlisted_dynamic_script_import=1
allowlisted_legacy_mixed=1
strict_script_wrapper=56
strict_workflows=15
total_test_files=269
violations=0
```

Remaining dynamic target:

- `tests/test_m061_s03.py`

## Write-path inventory baseline

```text
total_records=342
script-only=263
caller-owned=38
run-scoped=25
append-log=7
shared-state=4
temporary=1
database=1
unknown=3
```

Remaining unknown records:

```text
src/research_graph/cli/__init__.py
  L355 write_text target=paper_dir / 'paper.json'
  L358 write_text target=paper_dir / 'scored.json'

src/research_graph/infrastructure/corpus/ingestion/fetchers.py
  L44 write_bytes target=pdf_path
```

## Queue baseline

```text
uv run pytest tests/test_universal_kb_queue.py -q
24 passed
```

This includes M168's bounded thread-level separate-connection stress proof. M169 will add a bounded multiprocess proof if the queue APIs and runtime shape support it safely.

## Baseline risk notes

1. `tests/test_m061_s03.py` is baseline-green only while allowlisted. A normal-import migration previously exposed historical artifact hash and summary drift.
2. Unknown write paths are few enough to review manually, but their ownership must be established before scanner classification or atomic hardening.
3. Queue tests are fast and green; multiprocess stress still needs a bounded design to avoid flake or slow closeout.
