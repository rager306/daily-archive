# M072 Evaluation Report

## Summary

M072 evaluates metadata-only train and validation fixtures with the M071 deterministic evaluator. These metrics validate benchmark mechanics over reviewed metadata labels; they do not claim production extraction quality.

## Results

### train

| Metric | Value |
|---|---:|
| `case_count` | 6 |
| `entity_f1` | 0.9166666666666666 |
| `relation_f1` | 0.8333333333333334 |
| `evidence_path_validity` | 0.9444444444444444 |
| `schema_validity` | 0.8333333333333334 |
| `json_validity` | 1.0 |
| `mean_cost_estimate` | 0.035 |
| `mean_latency_ms` | 350.0 |
| `total_retry_count` | 3 |

### validation

| Metric | Value |
|---|---:|
| `case_count` | 3 |
| `entity_f1` | 0.8333333333333334 |
| `relation_f1` | 0.4 |
| `evidence_path_validity` | 0.875 |
| `schema_validity` | 0.6666666666666666 |
| `json_validity` | 1.0 |
| `mean_cost_estimate` | 0.04 |
| `mean_latency_ms` | 300.0 |
| `total_retry_count` | 3 |

## Readiness verdict

The benchmark gate is ready for a future baseline extraction spike, but **not** for DSPy/MiniMax optimization yet. A future milestone should first expand labels beyond title metadata and add full-paper evidence paths.

## Limitations

- Labels are reviewed metadata-title labels, not human-reviewed full-paper annotations.
- Baseline predictions are deterministic fixtures, not model outputs.
- No n-ary/hyperedge scoring yet.
- No LLM judge or MiniMax/DSPy calls.
- No graph writes or fact promotion.
