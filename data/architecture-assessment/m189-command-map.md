# M189 Command Map

## Verdict

**M189 can use existing local benchmark, ablation, and DSPy-boundary tests as representative gates without editing source.**

## Discovery evidence

- Command and test surface discovery: `gsd_exec[0c81a923-d4bf-4c20-8be0-49e7a2547de1]`

## Present surfaces

- `tests/test_evaluation_benchmark.py`
- `tests/test_extraction_benchmark.py`
- `tests/test_dspy_extraction_boundary.py`
- `src/research_graph/infrastructure/evaluation/evaluation_metrics.py`
- `src/research_graph/infrastructure/evaluation/dspy_extraction.py`
- `src/research_graph/infrastructure/retrieval/hybrid.py`
- `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py`
- `data/architecture-assessment/m188-final-validation-evidence.md`

## Representative commands

S02 metric contract gates:

```bash
uv run pytest tests/test_extraction_benchmark.py -q
uv run pytest tests/test_evaluation_benchmark.py -q -k 'schema_validity or groundedness_proxy or evidence_path_hit_rate or retrieval_recall'
```

S03 ablation protocol gates:

```bash
uv run pytest tests/test_evaluation_benchmark.py -q -k 'retrieval_ablation_runner'
uv run pytest tests/test_dspy_extraction_boundary.py -q
```

S04 final representative gate:

```bash
uv run pytest tests/test_extraction_benchmark.py tests/test_evaluation_benchmark.py tests/test_dspy_extraction_boundary.py -q
```

## Test inventory

Evaluation benchmark tests:

- `test_extraction_benchmark_fixture_schema_validity_is_clean`
- `test_groundedness_proxy_reports_expected_evidence_ids`
- `test_groundedness_proxy_names_missing_unexpected_and_none_evidence_ids`
- `test_evidence_path_hit_rate_handles_hits_misses_duplicates_and_none_ids`
- `test_evidence_path_hit_rate_empty_expected_sets_and_empty_results`
- `test_retrieval_recall_handles_duplicates_missing_ids_none_ids_and_empty_lists`
- `test_retrieval_ablation_runner_exercises_s05_fixture_and_s06_modes`
- `test_retrieval_ablation_runner_reports_empty_results_and_missing_ids`

Extraction benchmark tests:

- `test_smoke_fixture_metrics_match_expected`
- `test_m072_reviewed_fixture_metrics_match_expected`
- `test_perfect_records_score_one`
- `test_invalid_prediction_schema_reduces_validity_without_crashing`
- `test_gold_fixture_validation_is_strict`
- `test_benchmark_metrics_can_be_stored_in_queue_payload_metadata`

## Future-only surfaces

These are documentation targets only in M189 unless a later milestone performs exact GitNexus impact first:

- `src/research_graph/infrastructure/evaluation/dspy_extraction.py`
- `src/research_graph/infrastructure/retrieval/hybrid.py`
- `src/research_graph/infrastructure/graph/ladybug_client.py`

## Guardrails

- Tests are representative gates, not production quality claims.
- DSPy boundary tests may be run, but no DSPy optimization is enabled.
- Hybrid retrieval remains deterministic fixture-level baseline.
- Graph import and production persistence remain out of scope.
