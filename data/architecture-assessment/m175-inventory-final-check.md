# M175 Final Inventory Check

## Verdict

**PASS.**

## Expected counts

| Category | Count |
|---|---:|
| daily-cli-output | 5 |
| validation-batch-output | 10 |
| caller-owned | 10 |
| run-scoped | 6 |
| append-log | 1 |
| temporary | 1 |
| script-only | 265 |
| shared-state | 0 |
| unknown | 0 |

## Totals

- `total_records=341`.
- `by_root.scripts=265`.
- `by_root.src=76`.

## Generated delta assertions

- `daily-cli-output +5`.
- `validation-batch-output +10`.
- `caller-owned -10`.
- `run-scoped -5`.
- `script-only +1` from the scanner delta writer.
- `unknown=0` and `shared-state=0`.
