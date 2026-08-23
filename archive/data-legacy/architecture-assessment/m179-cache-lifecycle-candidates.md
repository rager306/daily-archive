# M179 Cache Lifecycle Candidates

## Summary

```text
article-artifact-package=2
caller-owned=3
caller-owned-index=1
figure-extraction-benchmark-output=3
graph-readiness-evidence=1
inventory-report-output=2
m057-structure-extraction-output=2
m060-graph-figure-benchmark-output=4
m061-acquisition-pipeline-output=3
parser-replay-output=1
r024-conversion-output=3
script-only=16
source-asset-package=1
validation-batch-output=1
```

## Candidate records

| Category | Path | Line | Operation | Target | Reason |
|---|---|---:|---|---|---|
| `article-artifact-package` | `src/research_graph/cli/commands/article_artifacts.py` | 398 | `write_text` | `manifest_path` | reviewed article artifact package output |
| `article-artifact-package` | `src/research_graph/infrastructure/papers/artifacts/metrics.py` | 293 | `write_text` | `markdown_path` | reviewed article artifact package output |
| `caller-owned` | `src/research_graph/infrastructure/corpus/reporting/coverage_report.py` | 45 | `write_text` | `self.markdown_path` | caller-provided or adapter-owned output path |
| `caller-owned` | `src/research_graph/infrastructure/corpus/sources/markdown_converter.py` | 347 | `write_text` | `md_path` | caller-provided or adapter-owned output path |
| `caller-owned` | `src/research_graph/infrastructure/corpus/sources/markdown_converter.py` | 348 | `write_text` | `method_path` | caller-provided or adapter-owned output path |
| `caller-owned-index` | `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | 183 | `write_text` | `index_path` | caller-provided paired review index output |
| `figure-extraction-benchmark-output` | `scripts/m058_compare_v2_vs_m057.py` | 179 | `write_text` | `output_md_path` | reviewed figure extraction benchmark output |
| `figure-extraction-benchmark-output` | `scripts/m058_marker_compare_5.py` | 318 | `write_text` | `COMPARISON_MD` | reviewed figure extraction benchmark output |
| `figure-extraction-benchmark-output` | `scripts/m058_marker_compare_5.py` | 319 | `write_text` | `DECISION_MD` | reviewed figure extraction benchmark output |
| `graph-readiness-evidence` | `src/research_graph/infrastructure/graph/readiness/manifest.py` | 79 | `write_text` | `output_path` | reviewed graph-readiness evidence output |
| `inventory-report-output` | `scripts/inventory_write_paths.py` | 386 | `write_text` | `args.markdown` | reviewed inventory report output |
| `inventory-report-output` | `scripts/inventory_write_paths.py` | 390 | `write_text` | `args.delta_markdown` | reviewed inventory report output |
| `m057-structure-extraction-output` | `scripts/m057_build_graph_manifest.py` | 41 | `write_text` | `path` | reviewed M057 structure extraction output |
| `m057-structure-extraction-output` | `scripts/m057_compare_marker_opendataloader.py` | 238 | `write_text` | `md_output` | reviewed M057 structure extraction output |
| `m060-graph-figure-benchmark-output` | `scripts/m060b_graph_stats.py` | 245 | `write_text` | `md_path` | reviewed M060 graph and figure benchmark output |
| `m060-graph-figure-benchmark-output` | `scripts/m060b_graph_validate.py` | 292 | `write_text` | `md_path` | reviewed M060 graph and figure benchmark output |
| `m060-graph-figure-benchmark-output` | `scripts/m060c_applicability_matrix.py` | 473 | `write_text` | `markdown_path` | reviewed M060 graph and figure benchmark output |
| `m060-graph-figure-benchmark-output` | `scripts/m060c_benchmark.py` | 397 | `write_text` | `md_path` | reviewed M060 graph and figure benchmark output |
| `m061-acquisition-pipeline-output` | `scripts/m061_anchor_pilot.py` | 846 | `write_text` | `parser_dir / 'opendataloader.md'` | reviewed M061 acquisition pipeline output |
| `m061-acquisition-pipeline-output` | `scripts/m061_full_5_anchors.py` | 739 | `write_text` | `BASE_OUTPUT_DIR / 's02-decision.md'` | reviewed M061 acquisition pipeline output |
| `m061-acquisition-pipeline-output` | `scripts/m061_full_5_anchors.py` | 761 | `write_text` | `BASE_OUTPUT_DIR / 's02-decision.md'` | reviewed M061 acquisition pipeline output |
| `parser-replay-output` | `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` | 257 | `write_text` | `cache_path` | reviewed parser replay output |
| `r024-conversion-output` | `scripts/convert_r024_53_pdf_to_text.py` | 70 | `write_text` | `out_path` | reviewed R024 conversion output |
| `r024-conversion-output` | `scripts/convert_r024_53_pdf_to_text.py` | 105 | `open` | `EVENTS_LOG` | reviewed R024 conversion output |
| `r024-conversion-output` | `scripts/convert_r024_53_pdf_to_text.py` | 119 | `write_text` | `SUMMARY` | reviewed R024 conversion output |
| `script-only` | `scripts/audit_test_architecture.py` | 205 | `write_text` | `markdown_path` | write occurs in process-boundary script |
| `script-only` | `scripts/benchmark_m055_corpus_manifest.py` | 118 | `write_text` | `output_path` | write occurs in process-boundary script |
| `script-only` | `scripts/build_m055deep_corpus_manifest_20.py` | 224 | `write_text` | `output_path` | write occurs in process-boundary script |
| `script-only` | `scripts/m052_rlm_e2e.py` | 312 | `write_text` | `audit_md_path` | write occurs in process-boundary script |
| `script-only` | `scripts/m058_build_graph_manifest.py` | 53 | `write_text` | `path` | write occurs in process-boundary script |
| `script-only` | `scripts/m059_build_manifest.py` | 179 | `write_text` | `actual_output` | write occurs in process-boundary script |
| `script-only` | `scripts/m066_graphdb_full_benchmark.py` | 676 | `write_text` | `artifact_dir / 'scoring-matrix.md'` | write occurs in process-boundary script |
| `script-only` | `scripts/probe_m033_opendataloader_adaptix_adapter.py` | 301 | `write_text` | `output_dir / 'adaptix-adapter-report.md'` | write occurs in process-boundary script |
| `script-only` | `scripts/render_bounded_repair_prototype.py` | 92 | `write_text` | `markdown_output` | write occurs in process-boundary script |
| `script-only` | `scripts/render_chunk_repair_contract.py` | 86 | `write_text` | `markdown_output` | write occurs in process-boundary script |
| `script-only` | `scripts/test_fd_contract.py` | 1533 | `write_text` | `artifact_dir / REPORT_MD` | write occurs in process-boundary script |
| `script-only` | `scripts/test_fd_contract.py` | 1569 | `write_text` | `artifact_dir / GAP_MD` | write occurs in process-boundary script |
| `script-only` | `scripts/verify_m029_unified_conversion_quality_boundary.py` | 114 | `open` | `fd` | write occurs in process-boundary script |
| `script-only` | `scripts/verify_m031_parser_conversion_replay.py` | 107 | `write_text` | `path` | write occurs in process-boundary script |
| `script-only` | `scripts/verify_m033_opendataloader_adaptix_adapter.py` | 173 | `write_text` | `adapter_dir / 'adaptix-adapter-closeout-report.md'` | write occurs in process-boundary script |
| `script-only` | `scripts/verify_test_architecture.py` | 189 | `write_text` | `markdown_path` | write occurs in process-boundary script |
| `source-asset-package` | `src/research_graph/infrastructure/papers/source_assets/registry.py` | 452 | `write_text` | `manifests_dir / f"{manifest['paper_id']}-source-assets.json"` | reviewed source asset package output |
| `validation-batch-output` | `src/research_graph/workflows/validation/batch_workflow.py` | 138 | `write_text` | `selection_manifest_path` | reviewed validation batch output |

## Review rule

This list is for lifecycle review only. Scanner movement still requires exact source path plus clear lifecycle ownership proof.
