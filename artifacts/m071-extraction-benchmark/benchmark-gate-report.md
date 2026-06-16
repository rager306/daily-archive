# M071 Benchmark Gate Report

## Summary

M071 creates the first executable benchmark gate for future DSPy + MiniMax extraction work. It is deterministic, metadata-only, and runs locally without external API calls.

## Implemented artifacts

| Artifact | Purpose |
|---|---|
| `fixture-schema.md` | Documents metadata-only fixture schema and metrics. |
| `fixtures/smoke-gold.jsonl` | Gold labels for perfect, partial, and invalid cases. |
| `fixtures/smoke-predictions.jsonl` | Prediction fixture used to validate metrics. |
| `fixtures/smoke-expected-metrics.json` | Expected aggregate metrics for tests. |
| `src/arxiv_archive/extraction_benchmark.py` | Deterministic evaluator. |
| `tests/test_extraction_benchmark.py` | Evaluator and queue compatibility tests. |

## Metric gate

The evaluator reports:

- `entity_precision`
- `entity_recall`
- `entity_f1`
- `relation_precision`
- `relation_recall`
- `relation_f1`
- `evidence_path_validity`
- `schema_validity`
- `json_validity`
- `mean_cost_estimate`
- `mean_latency_ms`
- `total_retry_count`

Smoke fixture expected values:

| Metric | Expected |
|---|---:|
| `entity_f1` | 0.8 |
| `relation_f1` | 0.5 |
| `evidence_path_validity` | 0.8571428571428571 |
| `schema_validity` | 0.6666666666666666 |
| `json_validity` | 1.0 |
| `mean_cost_estimate` | 0.02 |
| `mean_latency_ms` | 200.0 |
| `total_retry_count` | 3 |

## Queue metadata mapping

The queue compatibility test stores benchmark results into M070 `payload_metadata.diagnostics` via `update_payload_diagnostics`:

| Evaluator output | Queue metadata destination |
|---|---|
| `entity_f1` | `payload_metadata.diagnostics.entity_f1` |
| `relation_f1` | `payload_metadata.diagnostics.relation_f1` |
| `evidence_path_validity` | `payload_metadata.diagnostics.evidence_path_validity` |
| `schema_validity` | `payload_metadata.diagnostics.schema_validity` |
| `json_validity` | `payload_metadata.diagnostics.json_validity` |
| `mean_cost_estimate` | `payload_metadata.cost_estimate` |
| `mean_latency_ms` | `payload_metadata.latency_ms` |
| `total_retry_count` | `payload_metadata.retry_count` |

`write_eligibility` remains `false` and `promotion_eligibility` remains `false`.

## Safety boundaries

M071 does not execute:

- MiniMax API calls,
- DSPy optimizers,
- Qwen local models,
- FalkorDB writes,
- fact promotion,
- distributed workers.

Fixtures are metadata-only and exclude raw article text, prompts, embeddings, vectors, API keys, and model payloads.

## Remaining steps before DSPy + MiniMax

Before running DSPy or MiniMax, future work still needs:

1. A larger reviewed train/validation fixture set over canonical PDFs.
2. Per-type F1 breakdown for Method, Task, Dataset, Metric, Limitation, Claim.
3. n-ary claim / hyperedge scoring.
4. Optional LLM judge rubric for scientific QA.
5. Budget thresholds for cost and latency.
6. A baseline MiniMax prompt run recorded as research-only artifacts.

## Verdict

M071 satisfies the executable benchmark gate foundation. Future DSPy + MiniMax work can now be planned against deterministic metrics, but actual optimization remains deferred until the user explicitly authorizes that milestone.
