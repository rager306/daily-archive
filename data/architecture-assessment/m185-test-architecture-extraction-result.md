# M185 Test Architecture Extraction Result

## Verdict

**PASS: extracted.**

## Movement

Moved concrete test architecture inventory logic from:

`scripts/audit_test_architecture.py`

to:

`src/research_graph/application/test_architecture_inventory.py`

The script remains a thin CLI wrapper and re-exports the previous public names needed by `scripts/verify_test_architecture.py` and tests.

## Scanner impact

Moving `write_outputs` changed root ownership from script to source, but the exact category remains `test-architecture-audit-output`. An exact source-path rule was added for `src/research_graph/application/test_architecture_inventory.py`; no broad source, target-name, output, path, or report rule was added.

## Evidence

| Check | Result | Evidence |
|---|---|---|
| GitNexus impact | LOW exact for moved symbols; `_classify` CRITICAL warned and narrowly edited | tool outputs in S03 |
| Focused tests | PASS: 44 passed | `gsd_exec[f4881b7d-061f-4eda-b60d-d8665d530a8d]` |
| CLI smoke | PASS | `gsd_exec[18bb692f-5c3a-4ea2-b69e-0133b3c4c035]` |
| Test architecture guard | PASS: violations=0 | `gsd_exec[3c416aac-a04e-4f01-98cb-0d496c1ab1ec]` |
| Write-path drift | PASS: unknown=0, shared-state=0, script-only<=4, category totals unchanged | `gsd_exec[5114a4b4-ffcf-4901-b3b1-088670d89f38]` |
| Ruff | PASS | `gsd_exec[0f6c57a0-9556-48d5-a771-4ea6377ad444]` |
| Pyrefly | PASS: 0 errors | `gsd_exec[47e48e40-0a0b-41de-b4c8-4c4ec982a7c2]` |

## Known limits

This slice only moved one concrete inventory module. It did not alter guardrail policy or manifest/cache residual decisions.
