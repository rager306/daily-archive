# M173 Final Inventory Check

## Verdict

**PASS.**

## Counts

| Category | Count |
|---|---:|
| append-log | 3 |
| article-artifact-package | 7 |
| caller-owned | 21 |
| caller-owned-index | 1 |
| database | 1 |
| graph-probe-output | 2 |
| graph-readiness-evidence | 14 |
| legacy-evidence-regeneration | 2 |
| parser-replay-output | 3 |
| run-owned-state | 1 |
| run-scoped | 13 |
| script-only | 264 |
| source-asset-package | 4 |
| source-scan-output | 3 |
| temporary | 1 |

## Assertions

- `total_records=340`.
- `unknown=0`.
- `parser-replay-output=3`.
- `source-scan-output=3`.
- `graph-probe-output=2`.
- Conservative broad buckets remain present: `caller-owned=21`, `run-scoped=13`, `append-log=3`.
