# M178 Quality Stack

## Verdict

**Quality stack status: PASS.**

## Results

| Check | Result | Evidence |
|---|---|---|
| Scoped ruff | PASS | `gsd_exec[ce888595-821d-4dd2-8308-aaaa5d71f0b5]` |
| Pyrefly | PASS: 0 errors | `gsd_exec[2f792780-8a43-4aae-ae5f-4edb7da929dc]` |
| Pre-commit | PASS | `gsd_exec[ebe136b8-1760-47aa-a0c6-4c054bcffe6a]` |
| GitNexus detect_changes | PASS: LOW risk, affected_processes=0 | tool output in S12 |
| Scope hygiene | PASS: expected M178 files only plus ignored runtime dirs | `gsd_exec[fb4ebd73-ae44-4d8a-a4d4-a4e4c25ed813]` |

## GitNexus summary

```text
changed_count=0
affected_count=0
changed_files=5
risk_level=low
affected_processes=[]
```

Pre-edit impact was UNKNOWN because scanner/workflow targets did not resolve authoritatively. Final detect_changes is LOW risk with no affected processes.

## Scope hygiene

Expected tracked changes:

- `.github/workflows/architecture-guardrail.yml`
- `.gsd/DECISIONS.md`
- `.gsd/ROADMAP.md`
- `scripts/inventory_write_paths.py`
- `tests/test_inventory_write_paths.py`
- `data/architecture-assessment/m178-*`

Expected ignored runtime noise:

- `.gsd/milestones/M178-w4s1zh/`
- `tmp/`
