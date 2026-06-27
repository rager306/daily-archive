# M185 Pipeline Audit Extraction Result

## Verdict

**PASS: extracted.**

## Movement

Moved the explicit recurring pipeline inventory builder, writer, and contract-list construction from:

`scripts/audit_pipeline_scripts.py`

to:

`src/research_graph/application/pipeline_script_audit_inventory.py`

Kept `src/research_graph/application/pipeline_script_inventory.py` as the data-contract module. The script remains a thin CLI wrapper and keeps `print_summary` local because printing is a CLI concern.

## Evidence

| Check | Result | Evidence |
|---|---|---|
| GitNexus impact | LOW exact for `_contract`, `_item`, `build_inventory`, `write_inventory`, `print_summary` | tool outputs in S04 |
| Focused pipeline tests | PASS: 15 passed | `gsd_exec[bf764f77-394c-4e81-b16a-090f79f7d0da]` |
| CLI smoke | PASS | `gsd_exec[f5b74d8f-eafa-4736-857b-7da67c9ca1be]` |
| Ruff | PASS | `gsd_exec[bbcd0236-ff61-43c1-89b3-0da8050e3738]` |
| Pyrefly | PASS: 0 errors | `gsd_exec[4c21314a-f051-4b6b-bd50-dd062ddd9411]` |
| Write-path drift | PASS: unknown=0, shared-state=0, script-only<=4 | `gsd_exec[d7e25673-5e9c-42c4-ae5e-70bb1a7236df]` |

## Fixes during verification

The first extraction pass exposed two issues: copied builder code referenced `REPO_ROOT`, and `print_summary` in the application module violated the no-print rule. The final version uses `Path(".")` as the app builder default and keeps `print_summary` in the CLI wrapper.
