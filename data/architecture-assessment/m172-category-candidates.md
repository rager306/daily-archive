# M172 Category Candidates

## Summary

| Candidate group | Existing category | Count |
|---|---|---:|
| article-artifact package outputs | append-log | 1 |
| article-artifact package outputs | caller-owned | 3 |
| article-artifact package outputs | run-scoped | 3 |
| cli outputs | caller-owned | 2 |
| cli outputs | run-scoped | 3 |
| graph probe outputs | caller-owned | 2 |
| graph-readiness evidence outputs | append-log | 2 |
| graph-readiness evidence outputs | caller-owned | 6 |
| graph-readiness evidence outputs | run-scoped | 6 |
| other broad outputs | append-log | 1 |
| other broad outputs | caller-owned | 16 |
| other broad outputs | run-scoped | 8 |
| parser replay outputs | caller-owned | 2 |
| parser replay outputs | run-scoped | 1 |
| repair benchmark outputs | append-log | 2 |
| repair benchmark outputs | caller-owned | 1 |
| repair benchmark outputs | run-scoped | 2 |
| source scan outputs | caller-owned | 5 |
| source-asset package outputs | append-log | 1 |
| source-asset package outputs | caller-owned | 1 |
| source-asset package outputs | run-scoped | 2 |

## Candidate details

### article-artifact package outputs / append-log

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/cli/commands/article_artifacts.py` | 400 | `diagnostics_path` |

### article-artifact package outputs / caller-owned

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/cli/commands/article_artifacts.py` | 399 | `run_summary_path` |
| `src/research_graph/infrastructure/papers/artifacts/metrics.py` | 292 | `json_path` |
| `src/research_graph/infrastructure/papers/artifacts/metrics.py` | 293 | `markdown_path` |

### article-artifact package outputs / run-scoped

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/cli/commands/article_artifacts.py` | 398 | `manifest_path` |
| `src/research_graph/infrastructure/papers/artifacts/batch_validation.py` | 582 | `report_path` |
| `src/research_graph/infrastructure/papers/artifacts/worker.py` | 273 | `target_path` |

### cli outputs / caller-owned

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/cli/__init__.py` | 232 | `filepath` |
| `src/research_graph/cli/__init__.py` | 348 | `filepath` |

### cli outputs / run-scoped

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/cli/__init__.py` | 442 | `day_dir / 'papers.json'` |
| `src/research_graph/cli/__init__.py` | 445 | `day_dir / 'scored.json'` |
| `src/research_graph/cli/__init__.py` | 448 | `day_dir / 'overview.json'` |

### graph probe outputs / caller-owned

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/infrastructure/graph/r024_networkx_probe.py` | 89 | `config.summary_path` |
| `src/research_graph/infrastructure/graph/r024_networkx_probe.py` | 94 | `config.memory_profile_path` |

### graph-readiness evidence outputs / append-log

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/infrastructure/graph/readiness/extraction_gate.py` | 69 | `events_path` |
| `src/research_graph/infrastructure/graph/readiness/retrieval_validation.py` | 142 | `output_dir / 'retrieval-validation-events.jsonl'` |

### graph-readiness evidence outputs / caller-owned

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/infrastructure/graph/readiness/export.py` | 147 | `summary_path` |
| `src/research_graph/infrastructure/graph/readiness/extraction_gate.py` | 66 | `summary_path` |
| `src/research_graph/infrastructure/graph/readiness/persistence.py` | 186 | `claims_path` |
| `src/research_graph/infrastructure/graph/readiness/persistence.py` | 196 | `summary_path` |
| `src/research_graph/infrastructure/graph/readiness/review.py` | 79 | `path` |
| `src/research_graph/infrastructure/graph/readiness/review.py` | 87 | `summary_path` |

### graph-readiness evidence outputs / run-scoped

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/infrastructure/graph/readiness/manifest.py` | 79 | `output_path` |
| `src/research_graph/infrastructure/graph/readiness/persistence.py` | 231 | `output_path` |
| `src/research_graph/infrastructure/graph/readiness/persistence.py` | 390 | `args.output` |
| `src/research_graph/infrastructure/graph/readiness/retrieval_validation.py` | 138 | `output_dir / 'retrieval-validation-results.json'` |
| `src/research_graph/infrastructure/graph/readiness/retrieval_validation.py` | 191 | `output_path` |
| `src/research_graph/infrastructure/graph/readiness/retrieval_validation.py` | 417 | `args.output` |

### other broad outputs / append-log

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/infrastructure/papers/chunking/chunker.py` | 685 | `output_dir / 'structure-aware-package-diagnostics.jsonl'` |

### other broad outputs / caller-owned

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/infrastructure/corpus/reporting/coverage_report.py` | 45 | `self.markdown_path` |
| `src/research_graph/infrastructure/corpus/reporting/coverage_report.py` | 46 | `self.json_path` |
| `src/research_graph/infrastructure/quality/gate.py` | 108 | `path` |
| `src/research_graph/infrastructure/quality/maintainability_report.py` | 72 | `path` |
| `src/research_graph/workflows/universal_kb/rehearsal.py` | 53 | `path` |
| `src/research_graph/workflows/universal_kb/smoke.py` | 101 | `path` |
| `src/research_graph/workflows/universal_kb/smoke_audit.py` | 51 | `path` |
| `src/research_graph/workflows/universal_kb/smoke_runner.py` | 56 | `path` |
| `src/research_graph/workflows/validation/batch_workflow.py` | 138 | `selection_manifest_path` |
| `src/research_graph/workflows/validation/batch_workflow.py` | 236 | `summary_path` |
| `src/research_graph/workflows/validation/batch_workflow.py` | 326 | `delta_path` |
| `src/research_graph/workflows/validation/batch_workflow.py` | 329 | `outlier_path` |
| `src/research_graph/workflows/validation/batch_workflow.py` | 446 | `path` |
| `src/research_graph/workflows/validation/batch_workflow.py` | 508 | `summary_path` |
| `src/research_graph/workflows/validation/batch_workflow.py` | 649 | `summary_path` |
| `src/research_graph/workflows/validation/batch_workflow.py` | 736 | `summary_path` |

### other broad outputs / run-scoped

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/application/validation/batch_provenance.py` | 247 | `output_path` |
| `src/research_graph/infrastructure/papers/chunking/chunker.py` | 681 | `output_dir / 'structure-aware-summary.json'` |
| `src/research_graph/infrastructure/staging/graph_candidates.py` | 375 | `output_path` |
| `src/research_graph/infrastructure/staging/import_boundary.py` | 386 | `summary_file` |
| `src/research_graph/workflows/universal_kb/smoke_audit.py` | 267 | `output_path` |
| `src/research_graph/workflows/universal_kb/smoke_selection.py` | 170 | `args.output` |
| `src/research_graph/workflows/validation/batch_workflow.py` | 470 | `output_path` |
| `src/research_graph/workflows/validation/batch_workflow.py` | 493 | `output_path` |

### parser replay outputs / caller-owned

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` | 257 | `cache_path` |
| `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` | 357 | `self.summary_path` |

### parser replay outputs / run-scoped

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` | 303 | `output_path` |

### repair benchmark outputs / append-log

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | 128 | `diagnostics_path` |
| `src/research_graph/infrastructure/repair/chunking_benchmark.py` | 187 | `output_dir / 'chunking-benchmark-diagnostics.jsonl'` |

### repair benchmark outputs / caller-owned

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | 182 | `review_path` |

### repair benchmark outputs / run-scoped

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | 135 | `output_dir / 'baseline-summary.json'` |
| `src/research_graph/infrastructure/repair/chunking_benchmark.py` | 182 | `output_dir / 'chunking-benchmark-summary.json'` |

### source scan outputs / caller-owned

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/infrastructure/corpus/sources/markdown_converter.py` | 347 | `md_path` |
| `src/research_graph/infrastructure/corpus/sources/markdown_converter.py` | 348 | `method_path` |
| `src/research_graph/infrastructure/corpus/sources/thirty_paper_deviation_scan.py` | 92 | `summary_path` |
| `src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py` | 114 | `destination` |
| `src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py` | 149 | `summary_path` |

### source-asset package outputs / append-log

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/infrastructure/papers/source_assets/registry.py` | 442 | `output_dir / 'source-asset-package-diagnostics.jsonl'` |

### source-asset package outputs / caller-owned

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/infrastructure/papers/source_assets/registry.py` | 452 | `manifests_dir / f"{manifest['paper_id']}-source-assets.json"` |

### source-asset package outputs / run-scoped

| Path | Line | Target |
|---|---:|---|
| `src/research_graph/infrastructure/papers/source_assets/registry.py` | 434 | `output_dir / 'source-preservation-summary.json'` |
| `src/research_graph/infrastructure/papers/source_assets/registry.py` | 438 | `output_dir / 'source-asset-summary.json'` |

## Non-candidate groups for now

- `other broad outputs`: mixed paths; keep existing categories until individually reviewed.
- `cli outputs`: mixed user-selected output targets; do not split without command-level review.
- `parser replay outputs`, `source scan outputs`, `graph probe outputs`, and `repair benchmark outputs`: viable later, but not first implementation group unless S03 approves exact path families.
- Any future generic `state`, `index`, `catalog`, queue, or cache target remains conservative unless reviewed by exact path.
