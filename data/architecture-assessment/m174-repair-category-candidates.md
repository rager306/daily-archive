# M174 Repair Category Candidates

## Summary

| Role | Count |
|---|---:|
| movable repair benchmark records | 5 |
| preserved caller-owned-index exception | 1 |

## Exact records

| Path | Line | Current category | Target | M174 role |
|---|---:|---|---|---|
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | 128 | `append-log` | `diagnostics_path` | candidate repair-benchmark-output |
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | 135 | `run-scoped` | `output_dir / 'baseline-summary.json'` | candidate repair-benchmark-output |
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | 182 | `caller-owned` | `review_path` | candidate repair-benchmark-output |
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | 183 | `caller-owned-index` | `index_path` | preserve caller-owned-index |
| `src/research_graph/infrastructure/repair/chunking_benchmark.py` | 182 | `run-scoped` | `output_dir / 'chunking-benchmark-summary.json'` | candidate repair-benchmark-output |
| `src/research_graph/infrastructure/repair/chunking_benchmark.py` | 187 | `append-log` | `output_dir / 'chunking-benchmark-diagnostics.jsonl'` | candidate repair-benchmark-output |

## Boundaries

- This artifact does not approve categories; S03 freezes scope.
- `index_path` in `chunk_baseline_measurement.py` must remain `caller-owned-index`.
- Do not classify by target names like `diagnostics`, `summary`, `review`, or `benchmark` globally.
- Do not classify all `src/research_graph/infrastructure/repair/` writes as repair benchmark outputs.
- Other repair modules remain unchanged until separately reviewed.
