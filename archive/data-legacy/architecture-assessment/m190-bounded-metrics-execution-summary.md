# M190 Bounded Metrics Execution Summary

## Verdict

**PASS: bounded M190 execution met the pre-written expected output contract within the M027 local six-article scope.**

## Expected-vs-observed map

| Expected label | Observed result | Evidence |
|---|---|---|
| `source_quality_labels_present` | PASS: M027 source acquisition summary status `captured`, article_count=6 | `gsd_exec[6aefb55f-ddbc-4b35-b232-3bd37ef45b82]` |
| `low_quality_source_fail_closed` | PASS: focused low-quality source criteria tests 4 passed | `m190-s03-metric-ablation-results.md` |
| `parser_ready_scope` | PASS: bounded M027 conversion summary reports parser_ready_count=6 | `gsd_exec[6aefb55f-ddbc-4b35-b232-3bd37ef45b82]` |
| `chunk_ready_scope` | PASS: M027 current pipeline replay wrote 6 per-article baseline JSON files | `m190-s03-local-validator-results.md` |
| `extraction_metric_gate_passed` | PASS: combined metric suite included extraction benchmark tests; total 23 passed | `m190-s03-metric-ablation-results.md` |
| `retrieval_ablation_gate_passed` | PASS: combined metric suite included evaluation benchmark ablation tests; total 23 passed | `m190-s03-metric-ablation-results.md` |
| `dspy_boundary_gate_passed` | PASS: combined metric suite included DSPy boundary tests; total 23 passed | `m190-s03-metric-ablation-results.md` |
| `graph_import_ready=false` | PASS: unsafe summary flags absent and validators preserve fail-closed graph/import claims | `gsd_exec[6aefb55f-ddbc-4b35-b232-3bd37ef45b82]` |
| `production_persistence_ready=false` | PASS: unsafe summary flags absent and no production persistence was written | `gsd_exec[6aefb55f-ddbc-4b35-b232-3bd37ef45b82]` |
| `optimizer_enabled=false` | PASS: no optimizer output or invocation occurred | `m190-s03-metric-ablation-results.md` |
| `direct_extractor_to_graph_write=false` | PASS: no source code changed and no graph write artifact was produced | Git status and GitNexus evidence |

## Generated artifact scope

M190 generated new execution artifacts under:

- `data/architecture-assessment/m190-m027-current-pipeline-replay/`

Running the M027 replay/verifier also refreshed generated M027 evidence artifacts:

- `data/article_corpora/m027-mixed-source-corpus-v1/conversion-quality-report.md`
- `data/article_corpora/m027-mixed-source-corpus-v1/conversion-quality-summary.json`
- `data/article_corpora/m027-mixed-source-corpus-v1/current-pipeline-baseline-diagnostics.jsonl`
- `data/article_corpora/m027-mixed-source-corpus-v1/current-pipeline-baseline-report.md`
- `data/article_corpora/m027-mixed-source-corpus-v1/current-pipeline-baseline-summary.json`
- `data/article_corpora/m027-mixed-source-corpus-v1/source-acquisition-report.md`
- `data/article_corpora/m027-mixed-source-corpus-v1/source-acquisition-summary.json`

## GitNexus result

GitNexus detect_changes after S03 execution:

- risk: LOW
- affected processes: 0
- changed symbols: generated M027 report sections only

## Boundary statement

M190 may claim bounded execution evidence for the M027 local six-article scope. It must not claim broad parser readiness, graph import readiness, production persistence readiness, production hybrid retrieval quality, or DSPy/RLM optimizer readiness.
