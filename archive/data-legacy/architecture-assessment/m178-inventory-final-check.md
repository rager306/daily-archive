# M178 Final Inventory Check

## Verdict

**PASS.**

## Expected counts

| Category | Count |
|---|---:|
| script-only | 170 |
| unknown | 0 |
| shared-state | 0 |
| m027-pipeline-replay-output | 14 |
| m025-recovery-evidence-output | 14 |
| inventory-report-output | 3 |
| queue-gate-output | 2 |
| smoke-script-output | 8 |

## Totals

- `total_records=341`.
- `by_root.scripts=265`.
- `by_root.src=76`.

## Generated delta assertions

- `script-only -28`.
- `m027-pipeline-replay-output +14`.
- `m025-recovery-evidence-output +14`.
- `unknown=0` and `shared-state=0`.
