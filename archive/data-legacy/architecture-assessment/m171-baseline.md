# M171 Baseline

## Verdict

**Baseline status: PASS.**

M171 starts from a green post-M170 state and can proceed with all three requested tracks together.

## Test architecture guard

Evidence: `gsd_exec[e04b429f-4bd5-44df-8eac-94d14c9ed9b9]`.

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

## Onion guard

Evidence: `gsd_exec[5869d5f1-a2a4-4e3b-8359-d228f7b1e1b8]`.

```text
violation_count=0
allowed_violation_count=0
```

## Write-path inventory

Evidence: `gsd_exec[4a60f3c0-f01a-44bf-9746-f773d5cb31f0]`.

Generated:

```text
data/architecture-assessment/m171-write-path-inventory-baseline.json
data/architecture-assessment/m171-write-path-inventory-baseline.md
```

Counts:

```text
total_records=340
script-only=264
caller-owned=38
run-scoped=25
append-log=7
shared-state=4
temporary=1
database=1
unknown=0
```

## Queue soak harness smoke

Evidence: `gsd_exec[aaee7e1c-5594-44b3-b39a-2d02e92873dc]`.

```text
jobs=4
processes=2
rounds=1
total_completed=4
unique_completed=4
worker_errors=[]
stuck_workers=[]
```

## Status hygiene

Evidence: `gsd_exec[26383dd7-d5bd-46ac-b731-48dfc73133fe]`.

Expected active GSD planning state:

```text
M .gsd/ROADMAP.md
!! .gsd/milestones/M171-7vt18j/
!! tmp/
```

No `.codebase-memory` or `artifacts/quality` drift appeared in filtered status.

## Baseline implications

- M171 must preserve dynamic=0 and legacy=0.
- Strict onion allowlist must remain empty.
- Inventory starts at unknown=0 and shared-state=4.
- M170 queue soak harness is operational.
