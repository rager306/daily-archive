# M173 Baseline

## Verdict

Current inventory is green and batch-two category expansion is feasible for exact reviewed path families.

## Current counts

```text
total_records=340
unknown=0
append-log=3
article-artifact-package=7
caller-owned=28
caller-owned-index=1
database=1
graph-readiness-evidence=14
legacy-evidence-regeneration=2
run-owned-state=1
run-scoped=14
script-only=264
source-asset-package=4
temporary=1
```

Generated artifacts:

```text
data/architecture-assessment/m173-write-path-inventory-baseline.json
data/architecture-assessment/m173-write-path-inventory-baseline.md
```

## Batch-two candidates

| Candidate category | Existing categories | Count | Exact path scope |
|---|---|---:|---|
| `parser-replay-output` | caller-owned=2, run-scoped=1 | 3 | `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` |
| `source-scan-output` | caller-owned=3 | 3 | `src/research_graph/infrastructure/corpus/sources/thirty_paper_deviation_scan.py`, `src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py` |
| `graph-probe-output` | caller-owned=2 | 2 | `src/research_graph/infrastructure/graph/r024_networkx_probe.py` |

## Scope boundary

M173 should classify only these exact source path families. Do not classify by target tokens such as `summary_path`, `destination`, `cache_path`, or `memory_profile_path` globally.
