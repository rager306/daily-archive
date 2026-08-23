# M189 GitNexus Metrics Scope

## Verdict

**M189 scope is metrics and ablation design only. It does not activate DSPy, RLM optimization, graph import, or production persistence.**

## GitNexus planning basis

Queries used:

1. `real corpus expansion metrics ablation design evaluation scoring DSPy boundary optimizer guard scientific extraction`
2. `evaluation schema scoring statistical context metrics extraction signatures tests quality metrics ablation baseline fixtures`

## Candidate surfaces

| Surface | GitNexus symbol | M189 use | Boundary |
|---|---|---|---|
| Retrieval ablation tests | `Function:tests/test_evaluation_benchmark.py:test_retrieval_ablation_runner_exercises_s05_fixture_and_s06_modes` | Representative ablation gate. | Fixture/deterministic baseline only; not production retrieval quality. |
| Extraction benchmark tests | `Function:tests/test_extraction_benchmark.py:test_m072_reviewed_fixture_metrics_match_expected` | Representative metric gate. | Existing reviewed fixture metrics only; not optimizer activation. |
| DSPy signature boundary | `Function:src/research_graph/infrastructure/evaluation/dspy_extraction.py:dspy_extraction_signature_spec` | Boundary to explicitly keep inactive. | No DSPy optimizer, no live extraction expansion. |
| Hybrid retrieval | `Function:src/research_graph/infrastructure/retrieval/hybrid.py:retrieve_hybrid` | Ablation surface to document, not modify. | Deterministic fixture-level retrieval; no production graph import or persistence. |

## Existing process signals

GitNexus identified relevant process groups:

- `Compare_rlm_graph_traversal -> _empty_graph_diagnostics`
- `Compare_rlm_graph_traversal -> _evidence_paths_by_chunk`
- `Build_baseline_package -> _counts`
- `Build_baseline_package -> Node_by_id`

These are planning signals for metric and ablation contracts, not permission to change retrieval or graph code.

## Candidate tests

- `uv run pytest tests/test_evaluation_benchmark.py -q`
- `uv run pytest tests/test_extraction_benchmark.py -q`
- `uv run pytest tests/test_dspy_extraction_boundary.py -q`

## Non goals

M189 will not:

- tune, optimize, or activate DSPy;
- claim RLM or hybrid retrieval production quality;
- write graph/import state;
- alter `retrieve_hybrid`, `upsert_scientific_kg`, or DSPy modules;
- treat M188 `parser_ready=partial` as broad parser readiness;
- treat low-quality source or zero-chunk cases as success.

## Scope decision

Proceed with evidence-only metric and ablation design:

1. S02: metric contract baseline.
2. S03: ablation protocol baseline.
3. S04: final validation and next execution handoff.
