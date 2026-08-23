# M170 Baseline

## Verdict

**Baseline status: PASS.**

M170 starts from the post-M169 clean architecture baseline. The three requested tracks can begin without first repairing guardrail regressions.

## Guard counts

### Test architecture guard

Evidence: `gsd_exec[2759c009-baac-4209-a6d8-00702b950d6c]`.

```text
allowlisted_dynamic_script_import=0
allowlisted_legacy_mixed=0
strict_application=6
strict_domain=0
strict_infrastructure=6
strict_script_wrapper=57
strict_workflows=15
total_test_files=269
violations=0
```

### Onion guard

Evidence: `gsd_exec[bbfc7203-73c4-4096-afa7-6e97b30f442f]`.

```text
violation_count=0
allowed_violation_count=0
```

## Write-path inventory

Evidence: `gsd_exec[1cd59693-beba-4ad5-b041-e8709f3aa9fb]`.

Generated:

```text
data/architecture-assessment/m170-write-path-inventory.json
data/architecture-assessment/m170-write-path-inventory.md
```

Counts:

```text
total_records=339
script-only=263
caller-owned=38
run-scoped=25
append-log=7
shared-state=4
temporary=1
database=1
unknown=0
```

## Queue baseline

Evidence: `gsd_exec[8cc42e75-f4df-455f-8a88-0754bcbdc5e8]`.

```text
tests/test_universal_kb_queue.py: 25 passed
```

## Status baseline

Evidence: `gsd_exec[f6d58972-d9b2-44fd-873b-583fe47f83ef]`.

Expected current GSD planning drift:

```text
M .gsd/ROADMAP.md
!! .gsd/milestones/M170-kgl839/
!! tmp/
```

No `.codebase-memory` or `artifacts/quality` drift was present in the filtered status check.

## Baseline implications

- Architecture backlog work should preserve zero dynamic and zero legacy test allowlists.
- Strict onion allowlist starts empty and must remain empty.
- Write-path review starts from `unknown=0`; M170 should not hide shared-state records by broad classification.
- Queue work starts from a passing bounded process-level suite.
