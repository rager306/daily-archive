# M174 Baseline

## Verdict

Current inventory is green and repair benchmark category expansion is feasible if the existing `caller-owned-index` exception is preserved.

## Current counts

```text
total_records=340
unknown=0
append-log=3
article-artifact-package=7
caller-owned=21
caller-owned-index=1
database=1
graph-probe-output=2
graph-readiness-evidence=14
legacy-evidence-regeneration=2
parser-replay-output=3
run-owned-state=1
run-scoped=13
script-only=264
source-asset-package=4
source-scan-output=3
temporary=1
```

Generated artifacts:

```text
data/architecture-assessment/m174-write-path-inventory-baseline.json
data/architecture-assessment/m174-write-path-inventory-baseline.md
```

## Repair benchmark candidates

| Existing category | Count | Records |
|---|---:|---|
| append-log | 2 | repair diagnostics outputs |
| run-scoped | 2 | repair benchmark summary outputs |
| caller-owned | 1 | repair review output |
| caller-owned-index | 1 | existing reviewed index exception; preserve as-is |

## Candidate scope

Feasible category target: `repair-benchmark-output` for exact repair benchmark output records in:

- `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py`
- `src/research_graph/infrastructure/repair/chunking_benchmark.py`

Do not move `index_path`; it must remain `caller-owned-index`.
