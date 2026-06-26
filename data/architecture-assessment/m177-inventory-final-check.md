# M177 Final Inventory Check

## Verdict

**PASS.**

## Expected counts

| Category | Count |
|---|---:|
| script-only | 198 |
| unknown | 0 |
| shared-state | 0 |
| r024-corpus-selection-output | 6 |
| r024-entity-extraction-output | 3 |
| r024-conversion-output | 3 |
| r024-networkx-probe-output | 3 |
| r024-quality-metrics-output | 8 |
| inventory-report-output | 3 |
| queue-soak-output | 1 |
| queue-gate-output | 2 |
| smoke-script-output | 8 |
| caller-owned | 10 |
| run-scoped | 6 |
| database | 1 |

## Totals

- `total_records=341`.
- `by_root.scripts=265`.
- `by_root.src=76`.

## Generated delta assertions

- `script-only -37`.
- `r024-corpus-selection-output +6`.
- `r024-entity-extraction-output +3`.
- `r024-conversion-output +3`.
- `r024-networkx-probe-output +3`.
- `r024-quality-metrics-output +8`.
- `inventory-report-output +3`.
- `queue-soak-output +1`.
- `queue-gate-output +2`.
- `smoke-script-output +8`.
- `unknown=0` and `shared-state=0`.
