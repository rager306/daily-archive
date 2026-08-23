# M172 Baseline

## Verdict

Current write-path inventory is green and category expansion is feasible if limited to exact reviewed path groups.

## Current counts

```text
total_records=340
unknown=0
run-scoped=25
caller-owned=38
append-log=7
script-only=264
temporary=1
database=1
run-owned-state=1
legacy-evidence-regeneration=2
caller-owned-index=1
```

Generated artifacts:

```text
data/architecture-assessment/m172-write-path-inventory-baseline.json
data/architecture-assessment/m172-write-path-inventory-baseline.md
```

## Broad bucket candidates

Grouped candidates from broad categories:

| Existing category | Candidate group | Count |
|---|---|---:|
| caller-owned | graph-readiness | 6 |
| run-scoped | graph-readiness | 6 |
| append-log | graph-readiness | 2 |
| caller-owned | article-artifacts | 3 |
| run-scoped | article-artifacts | 3 |
| append-log | article-artifacts | 1 |
| caller-owned | source-assets | 1 |
| run-scoped | source-assets | 2 |
| append-log | source-assets | 1 |
| caller-owned | source-scans | 5 |
| caller-owned | graph-probes | 2 |
| run-scoped | repair-benchmarks | 2 |
| append-log | repair-benchmarks | 2 |

## Initial scope boundary

Do not split categories by broad target words alone. Only split exact path families where the role is clear and testable.

Likely first candidates:

1. graph-readiness evidence outputs;
2. source-asset package outputs;
3. article-artifact package outputs if exact targets stay clear.

Keep other records in existing broad categories until reviewed.
