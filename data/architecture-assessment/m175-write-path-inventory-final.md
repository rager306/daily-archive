# M167 Write Path Inventory

Schema: `daily-archive-write-path-inventory.v1`

## Summary

Total records: `341`

| Category | Count |
|---|---:|
| append-log | 1 |
| article-artifact-package | 7 |
| caller-owned | 10 |
| caller-owned-index | 1 |
| daily-cli-output | 5 |
| database | 1 |
| graph-probe-output | 2 |
| graph-readiness-evidence | 14 |
| legacy-evidence-regeneration | 2 |
| parser-replay-output | 3 |
| repair-benchmark-output | 5 |
| run-owned-state | 1 |
| run-scoped | 6 |
| script-only | 265 |
| source-asset-package | 4 |
| source-scan-output | 3 |
| temporary | 1 |
| validation-batch-output | 10 |

## Records

| Path | Line | Operation | Target | Category |
|---|---:|---|---|---|
| `src/research_graph/application/validation/batch_provenance.py` | 247 | `write_text` | `output_path` | run-scoped |
| `src/research_graph/application/validation/batch_state.py` | 252 | `write_text` | `output_path` | run-owned-state |
| `src/research_graph/cli/__init__.py` | 232 | `open` | `filepath` | daily-cli-output |
| `src/research_graph/cli/__init__.py` | 261 | `write_text` | `temp_path` | temporary |
| `src/research_graph/cli/__init__.py` | 348 | `write_text` | `filepath` | daily-cli-output |
| `src/research_graph/cli/__init__.py` | 442 | `write_text` | `day_dir / 'papers.json'` | daily-cli-output |
| `src/research_graph/cli/__init__.py` | 445 | `write_text` | `day_dir / 'scored.json'` | daily-cli-output |
| `src/research_graph/cli/__init__.py` | 448 | `write_text` | `day_dir / 'overview.json'` | daily-cli-output |
| `src/research_graph/cli/commands/article_artifacts.py` | 398 | `write_text` | `manifest_path` | article-artifact-package |
| `src/research_graph/cli/commands/article_artifacts.py` | 399 | `write_text` | `run_summary_path` | article-artifact-package |
| `src/research_graph/cli/commands/article_artifacts.py` | 400 | `write_text` | `diagnostics_path` | article-artifact-package |
| `src/research_graph/infrastructure/corpus/ingestion/catalog_adapters.py` | 540 | `write_text` | `summary_path` | legacy-evidence-regeneration |
| `src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py` | 935 | `write_text` | `report_path` | legacy-evidence-regeneration |
| `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` | 257 | `write_text` | `cache_path` | parser-replay-output |
| `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` | 303 | `write_text` | `output_path` | parser-replay-output |
| `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` | 357 | `write_text` | `self.summary_path` | parser-replay-output |
| `src/research_graph/infrastructure/corpus/reporting/coverage_report.py` | 45 | `write_text` | `self.markdown_path` | caller-owned |
| `src/research_graph/infrastructure/corpus/reporting/coverage_report.py` | 46 | `write_text` | `self.json_path` | caller-owned |
| `src/research_graph/infrastructure/corpus/sources/markdown_converter.py` | 347 | `write_text` | `md_path` | caller-owned |
| `src/research_graph/infrastructure/corpus/sources/markdown_converter.py` | 348 | `write_text` | `method_path` | caller-owned |
| `src/research_graph/infrastructure/corpus/sources/thirty_paper_deviation_scan.py` | 92 | `write_text` | `summary_path` | source-scan-output |
| `src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py` | 114 | `write_text` | `destination` | source-scan-output |
| `src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py` | 149 | `write_text` | `summary_path` | source-scan-output |
| `src/research_graph/infrastructure/graph/r024_networkx_probe.py` | 89 | `write_text` | `config.summary_path` | graph-probe-output |
| `src/research_graph/infrastructure/graph/r024_networkx_probe.py` | 94 | `write_text` | `config.memory_profile_path` | graph-probe-output |
| `src/research_graph/infrastructure/graph/readiness/export.py` | 147 | `write_text` | `summary_path` | graph-readiness-evidence |
| `src/research_graph/infrastructure/graph/readiness/extraction_gate.py` | 66 | `write_text` | `summary_path` | graph-readiness-evidence |
| `src/research_graph/infrastructure/graph/readiness/extraction_gate.py` | 69 | `write_text` | `events_path` | graph-readiness-evidence |
| `src/research_graph/infrastructure/graph/readiness/manifest.py` | 79 | `write_text` | `output_path` | graph-readiness-evidence |
| `src/research_graph/infrastructure/graph/readiness/persistence.py` | 186 | `write_text` | `claims_path` | graph-readiness-evidence |
| `src/research_graph/infrastructure/graph/readiness/persistence.py` | 196 | `write_text` | `summary_path` | graph-readiness-evidence |
| `src/research_graph/infrastructure/graph/readiness/persistence.py` | 231 | `write_text` | `output_path` | graph-readiness-evidence |
| `src/research_graph/infrastructure/graph/readiness/persistence.py` | 390 | `write_text` | `args.output` | graph-readiness-evidence |
| `src/research_graph/infrastructure/graph/readiness/retrieval_validation.py` | 138 | `write_text` | `output_dir / 'retrieval-validation-results.json'` | graph-readiness-evidence |
| `src/research_graph/infrastructure/graph/readiness/retrieval_validation.py` | 142 | `write_text` | `output_dir / 'retrieval-validation-events.jsonl'` | graph-readiness-evidence |
| `src/research_graph/infrastructure/graph/readiness/retrieval_validation.py` | 191 | `write_text` | `output_path` | graph-readiness-evidence |
| `src/research_graph/infrastructure/graph/readiness/retrieval_validation.py` | 417 | `write_text` | `args.output` | graph-readiness-evidence |
| `src/research_graph/infrastructure/graph/readiness/review.py` | 79 | `write_text` | `path` | graph-readiness-evidence |
| `src/research_graph/infrastructure/graph/readiness/review.py` | 87 | `write_text` | `summary_path` | graph-readiness-evidence |
| `src/research_graph/infrastructure/papers/artifacts/batch_validation.py` | 582 | `write_text` | `report_path` | article-artifact-package |
| `src/research_graph/infrastructure/papers/artifacts/metrics.py` | 292 | `write_text` | `json_path` | article-artifact-package |
| `src/research_graph/infrastructure/papers/artifacts/metrics.py` | 293 | `write_text` | `markdown_path` | article-artifact-package |
| `src/research_graph/infrastructure/papers/artifacts/worker.py` | 273 | `write_text` | `target_path` | article-artifact-package |
| `src/research_graph/infrastructure/papers/chunking/chunker.py` | 681 | `write_text` | `output_dir / 'structure-aware-summary.json'` | run-scoped |
| `src/research_graph/infrastructure/papers/chunking/chunker.py` | 685 | `write_text` | `output_dir / 'structure-aware-package-diagnostics.jsonl'` | append-log |
| `src/research_graph/infrastructure/papers/source_assets/registry.py` | 434 | `write_text` | `output_dir / 'source-preservation-summary.json'` | source-asset-package |
| `src/research_graph/infrastructure/papers/source_assets/registry.py` | 438 | `write_text` | `output_dir / 'source-asset-summary.json'` | source-asset-package |
| `src/research_graph/infrastructure/papers/source_assets/registry.py` | 442 | `write_text` | `output_dir / 'source-asset-package-diagnostics.jsonl'` | source-asset-package |
| `src/research_graph/infrastructure/papers/source_assets/registry.py` | 452 | `write_text` | `manifests_dir / f"{manifest['paper_id']}-source-assets.json"` | source-asset-package |
| `src/research_graph/infrastructure/quality/gate.py` | 108 | `write_text` | `path` | caller-owned |
| `src/research_graph/infrastructure/quality/maintainability_report.py` | 72 | `write_text` | `path` | caller-owned |
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | 128 | `write_text` | `diagnostics_path` | repair-benchmark-output |
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | 135 | `write_text` | `output_dir / 'baseline-summary.json'` | repair-benchmark-output |
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | 182 | `write_text` | `review_path` | repair-benchmark-output |
| `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | 183 | `write_text` | `index_path` | caller-owned-index |
| `src/research_graph/infrastructure/repair/chunking_benchmark.py` | 182 | `write_text` | `output_dir / 'chunking-benchmark-summary.json'` | repair-benchmark-output |
| `src/research_graph/infrastructure/repair/chunking_benchmark.py` | 187 | `write_text` | `output_dir / 'chunking-benchmark-diagnostics.jsonl'` | repair-benchmark-output |
| `src/research_graph/infrastructure/staging/graph_candidates.py` | 375 | `write_text` | `output_path` | run-scoped |
| `src/research_graph/infrastructure/staging/import_boundary.py` | 386 | `write_text` | `summary_file` | run-scoped |
| `src/research_graph/workflows/universal_kb/queue.py` | 120 | `sqlite3.connect` | `self.db_path` | database |
| `src/research_graph/workflows/universal_kb/rehearsal.py` | 53 | `write_text` | `path` | caller-owned |
| `src/research_graph/workflows/universal_kb/smoke.py` | 101 | `write_text` | `path` | caller-owned |
| `src/research_graph/workflows/universal_kb/smoke_audit.py` | 51 | `write_text` | `path` | caller-owned |
| `src/research_graph/workflows/universal_kb/smoke_audit.py` | 267 | `write_text` | `output_path` | run-scoped |
| `src/research_graph/workflows/universal_kb/smoke_runner.py` | 56 | `write_text` | `path` | caller-owned |
| `src/research_graph/workflows/universal_kb/smoke_selection.py` | 170 | `write_text` | `args.output` | run-scoped |
| `src/research_graph/workflows/validation/batch_workflow.py` | 138 | `write_text` | `selection_manifest_path` | validation-batch-output |
| `src/research_graph/workflows/validation/batch_workflow.py` | 236 | `write_text` | `summary_path` | validation-batch-output |
| `src/research_graph/workflows/validation/batch_workflow.py` | 326 | `write_text` | `delta_path` | validation-batch-output |
| `src/research_graph/workflows/validation/batch_workflow.py` | 329 | `write_text` | `outlier_path` | validation-batch-output |
| `src/research_graph/workflows/validation/batch_workflow.py` | 446 | `write_text` | `path` | validation-batch-output |
| `src/research_graph/workflows/validation/batch_workflow.py` | 470 | `write_text` | `output_path` | validation-batch-output |
| `src/research_graph/workflows/validation/batch_workflow.py` | 493 | `write_text` | `output_path` | validation-batch-output |
| `src/research_graph/workflows/validation/batch_workflow.py` | 508 | `write_text` | `summary_path` | validation-batch-output |
| `src/research_graph/workflows/validation/batch_workflow.py` | 649 | `write_text` | `summary_path` | validation-batch-output |
| `src/research_graph/workflows/validation/batch_workflow.py` | 736 | `write_text` | `summary_path` | validation-batch-output |
| `scripts/acquire_linked_target_pdfs.py` | 114 | `write_bytes` | `tmp_path` | script-only |
| `scripts/acquire_linked_target_pdfs.py` | 279 | `write_text` | `log_path` | script-only |
| `scripts/acquire_m056_wave.py` | 92 | `write_text` | `tmp_path` | script-only |
| `scripts/acquire_m056_wave.py` | 196 | `write_bytes` | `tmp_path` | script-only |
| `scripts/analyze_m056_wave_1.py` | 49 | `write_text` | `tmp_path` | script-only |
| `scripts/analyze_m056_wave_1.py` | 56 | `write_text` | `tmp_path` | script-only |
| `scripts/analyze_m056_wave_2.py` | 50 | `write_text` | `tmp_path` | script-only |
| `scripts/analyze_m056_wave_2.py` | 57 | `write_text` | `tmp_path` | script-only |
| `scripts/analyze_m056_wave_3.py` | 51 | `write_text` | `tmp_path` | script-only |
| `scripts/analyze_m056_wave_3.py` | 58 | `write_text` | `tmp_path` | script-only |
| `scripts/analyze_m056_wave_4.py` | 53 | `write_text` | `tmp_path` | script-only |
| `scripts/analyze_m056_wave_4.py` | 60 | `write_text` | `tmp_path` | script-only |
| `scripts/analyze_m056_wave_5.py` | 54 | `write_text` | `tmp_path` | script-only |
| `scripts/analyze_m056_wave_5.py` | 61 | `write_text` | `tmp_path` | script-only |
| `scripts/analyze_m056_wave_6.py` | 56 | `write_text` | `tmp` | script-only |
| `scripts/audit_locator_evidence.py` | 202 | `write_text` | `destination` | script-only |
| `scripts/audit_locator_evidence.py` | 206 | `write_text` | `destination` | script-only |
| `scripts/audit_m042_connectivity_groups.py` | 31 | `write_text` | `path` | script-only |
| `scripts/audit_m042_connectivity_groups.py` | 38 | `write_text` | `path` | script-only |
| `scripts/audit_m053_grobid_pilot.py` | 198 | `write_text` | `output_path` | script-only |
| `scripts/audit_m054_pdf_acquisition.py` | 227 | `write_text` | `DEFAULT_AUDIT_PATH` | script-only |
| `scripts/audit_pipeline_scripts.py` | 355 | `write_text` | `path` | script-only |
| `scripts/audit_test_architecture.py` | 204 | `write_text` | `json_path` | script-only |
| `scripts/audit_test_architecture.py` | 205 | `write_text` | `markdown_path` | script-only |
| `scripts/audit_test_architecture.py` | 206 | `write_text` | `pilot_path` | script-only |
| `scripts/augment_m073_evidence_paths.py` | 44 | `write_text` | `path` | script-only |
| `scripts/augment_m073_evidence_paths.py` | 146 | `write_text` | `output` | script-only |
| `scripts/benchmark_m055_availability_probe.py` | 195 | `write_text` | `output_path` | script-only |
| `scripts/benchmark_m055_corpus_manifest.py` | 118 | `write_text` | `output_path` | script-only |
| `scripts/benchmark_m055_grobid_only.py` | 65 | `write_bytes` | `tmp_path` | script-only |
| `scripts/benchmark_m055_hybrid_routing.py` | 50 | `write_bytes` | `tmp_path` | script-only |
| `scripts/benchmark_m055_opendataloader_only.py` | 59 | `write_bytes` | `tmp_path` | script-only |
| `scripts/benchmark_m055_vendor_check.py` | 134 | `write_text` | `output_path` | script-only |
| `scripts/benchmark_m055deep_grobid_fulltext.py` | 56 | `write_bytes` | `tmp_path` | script-only |
| `scripts/benchmark_m055deep_hybrid_routing_20.py` | 50 | `write_bytes` | `tmp_path` | script-only |
| `scripts/benchmark_m055deep_opendataloader_correctness.py` | 45 | `write_text` | `path` | script-only |
| `scripts/build_m028_hermes_digest_projection.py` | 775 | `write_text` | `out_dir / DIGEST_FILENAME` | script-only |
| `scripts/build_m028_hermes_digest_projection.py` | 778 | `write_text` | `out_dir / REPORT_FILENAME` | script-only |
| `scripts/build_m028_pdf_acquisition_diagnostics.py` | 740 | `write_text` | `out_dir / EVENTS_FILENAME` | script-only |
| `scripts/build_m028_pdf_acquisition_diagnostics.py` | 743 | `write_text` | `out_dir / SUMMARY_FILENAME` | script-only |
| `scripts/build_m028_pdf_acquisition_diagnostics.py` | 746 | `write_text` | `out_dir / REPORT_FILENAME` | script-only |
| `scripts/build_m028_source_metadata_adapters.py` | 668 | `write_text` | `events_path` | script-only |
| `scripts/build_m028_source_metadata_adapters.py` | 671 | `write_text` | `summary_path` | script-only |
| `scripts/build_m028_universal_loader_evidence_bundles.py` | 758 | `write_text` | `out_dir / BUNDLES_FILENAME` | script-only |
| `scripts/build_m028_universal_loader_evidence_bundles.py` | 761 | `write_text` | `out_dir / SUMMARY_FILENAME` | script-only |
| `scripts/build_m028_universal_loader_evidence_bundles.py` | 764 | `write_text` | `out_dir / REPORT_FILENAME` | script-only |
| `scripts/build_m043_sidecar_packets.py` | 56 | `write_text` | `path` | script-only |
| `scripts/build_m043_sidecar_packets.py` | 63 | `write_text` | `path` | script-only |
| `scripts/build_m055deep_corpus_manifest_20.py` | 224 | `write_text` | `output_path` | script-only |
| `scripts/build_r024_20_document_corpus_selection.py` | 159 | `write_text` | `OUT_SELECTION` | script-only |
| `scripts/build_r024_20_document_corpus_selection.py` | 194 | `open` | `OUT_EVENTS` | script-only |
| `scripts/build_r024_20_document_corpus_selection.py` | 212 | `write_text` | `OUT_SUMMARY` | script-only |
| `scripts/build_r024_53_document_corpus_selection.py` | 186 | `write_text` | `OUT_SELECTION` | script-only |
| `scripts/build_r024_53_document_corpus_selection.py` | 221 | `open` | `OUT_EVENTS` | script-only |
| `scripts/build_r024_53_document_corpus_selection.py` | 239 | `write_text` | `OUT_SUMMARY` | script-only |
| `scripts/build_r024_entity_networkx_probe.py` | 271 | `write_text` | `SUMMARY` | script-only |
| `scripts/build_r024_entity_networkx_probe.py` | 286 | `write_text` | `MEMORY_PROFILE` | script-only |
| `scripts/build_r024_entity_networkx_probe.py` | 288 | `open` | `PROBE_EVENTS` | script-only |
| `scripts/capture_m025_article_sources.py` | 31 | `write_text` | `path` | script-only |
| `scripts/capture_m025_article_sources.py` | 108 | `write_bytes` | `target` | script-only |
| `scripts/capture_m027_mixed_source_sources.py` | 920 | `write_text` | `report_path` | script-only |
| `scripts/capture_m027_mixed_source_sources.py` | 925 | `write_text` | `report_path` | script-only |
| `scripts/check_project_trajectory.py` | 121 | `write_text` | `path` | script-only |
| `scripts/check_project_trajectory.py` | 128 | `write_text` | `path` | script-only |
| `scripts/compare_m055_header_vs_fulltext.py` | 30 | `write_text` | `tmp_path` | script-only |
| `scripts/convert_m027_source_quality_boundary.py` | 91 | `open` | `fd` | script-only |
| `scripts/convert_m029_unified_source_quality_boundary.py` | 97 | `open` | `fd` | script-only |
| `scripts/convert_r024_53_pdf_to_text.py` | 70 | `write_text` | `out_path` | script-only |
| `scripts/convert_r024_53_pdf_to_text.py` | 105 | `open` | `EVENTS_LOG` | script-only |
| `scripts/convert_r024_53_pdf_to_text.py` | 119 | `write_text` | `SUMMARY` | script-only |
| `scripts/emit_m056_candidate_edges.py` | 288 | `write_text` | `output` | script-only |
| `scripts/extract_r024_20_document_quality_metrics.py` | 96 | `write_text` | `METRICS` | script-only |
| `scripts/extract_r024_20_document_quality_metrics.py` | 138 | `write_text` | `COMPARISON` | script-only |
| `scripts/extract_r024_53_document_quality_metrics.py` | 102 | `write_text` | `METRICS` | script-only |
| `scripts/extract_r024_53_document_quality_metrics.py` | 145 | `write_text` | `COMPARISON` | script-only |
| `scripts/extract_r024_entity_quality_metrics.py` | 81 | `write_text` | `METRICS` | script-only |
| `scripts/extract_r024_entity_quality_metrics.py` | 137 | `write_text` | `COMPARISON` | script-only |
| `scripts/extract_r024_entity_scale_entities.py` | 205 | `write_text` | `article_file` | script-only |
| `scripts/extract_r024_entity_scale_entities.py` | 238 | `open` | `EVENTS_LOG` | script-only |
| `scripts/extract_r024_entity_scale_entities.py` | 261 | `write_text` | `SUMMARY` | script-only |
| `scripts/extract_r024_quality_metrics.py` | 128 | `write_text` | `METRICS` | script-only |
| `scripts/extract_r024_quality_metrics.py` | 174 | `write_text` | `COMPARISON` | script-only |
| `scripts/inventory_write_paths.py` | 293 | `write_text` | `args.json` | script-only |
| `scripts/inventory_write_paths.py` | 294 | `write_text` | `args.markdown` | script-only |
| `scripts/inventory_write_paths.py` | 298 | `write_text` | `args.delta_markdown` | script-only |
| `scripts/legacy/m057_table_embed.py` | 170 | `write_text` | `output_path` | script-only |
| `scripts/m052_rlm_e2e.py` | 309 | `write_text` | `audit_json_path` | script-only |
| `scripts/m052_rlm_e2e.py` | 312 | `write_text` | `audit_md_path` | script-only |
| `scripts/m057_build_graph_manifest.py` | 41 | `write_text` | `path` | script-only |
| `scripts/m057_compare_marker_opendataloader.py` | 237 | `write_text` | `json_output` | script-only |
| `scripts/m057_compare_marker_opendataloader.py` | 238 | `write_text` | `md_output` | script-only |
| `scripts/m057_compare_marker_opendataloader_1pdf.py` | 81 | `write_text` | `out_path` | script-only |
| `scripts/m057_fd_validate.py` | 271 | `write_text` | `output_path` | script-only |
| `scripts/m057_figure_caption_build.py` | 228 | `write_text` | `output_path` | script-only |
| `scripts/m057_figure_embed.py` | 143 | `write_text` | `output_path` | script-only |
| `scripts/m057_figure_similarity.py` | 132 | `write_text` | `edges_path` | script-only |
| `scripts/m057_figure_similarity.py` | 135 | `write_text` | `summary_path` | script-only |
| `scripts/m057_marker_extract.py` | 207 | `write_text` | `destination` | script-only |
| `scripts/m057_marker_extract.py` | 238 | `write_text` | `per_pdf_dir / f"{packet['arxiv_id']}.json"` | script-only |
| `scripts/m057_marker_extract.py` | 269 | `write_text` | `output_dir / 'summary.json'` | script-only |
| `scripts/m057_marker_extract_5.py` | 116 | `write_text` | `per_pdf_path` | script-only |
| `scripts/m057_marker_extract_5.py` | 144 | `write_text` | `summary_path` | script-only |
| `scripts/m057_table_similarity.py` | 143 | `write_text` | `edges_path` | script-only |
| `scripts/m057_table_similarity.py` | 146 | `write_text` | `summary_path` | script-only |
| `scripts/m057_table_text_build.py` | 284 | `write_text` | `output_path` | script-only |
| `scripts/m058_build_graph_manifest.py` | 53 | `write_text` | `path` | script-only |
| `scripts/m058_compare_v2_vs_m057.py` | 175 | `write_text` | `output_json_path` | script-only |
| `scripts/m058_compare_v2_vs_m057.py` | 179 | `write_text` | `output_md_path` | script-only |
| `scripts/m058_compare_v2_vs_m057.py` | 180 | `write_text` | `decision_path` | script-only |
| `scripts/m058_marker_compare_5.py` | 315 | `write_text` | `COMPARISON_JSON` | script-only |
| `scripts/m058_marker_compare_5.py` | 318 | `write_text` | `COMPARISON_MD` | script-only |
| `scripts/m058_marker_compare_5.py` | 319 | `write_text` | `DECISION_MD` | script-only |
| `scripts/m058_marker_extract_5.py` | 221 | `write_text` | `PER_PDF_DIR / f'{sample.arxiv_id}.json'` | script-only |
| `scripts/m058_marker_extract_5.py` | 264 | `write_text` | `OUTPUT_ROOT / 'summary.json'` | script-only |
| `scripts/m058_plotextractor_embed.py` | 135 | `write_text` | `output_path` | script-only |
| `scripts/m058_plotextractor_extract.py` | 122 | `write_bytes` | `tarball_path` | script-only |
| `scripts/m058_plotextractor_extract.py` | 362 | `write_text` | `per_pdf_dir / f'{arxiv_id}.json'` | script-only |
| `scripts/m058_plotextractor_extract.py` | 424 | `write_text` | `summary_path` | script-only |
| `scripts/m058_plotextractor_extract.py` | 425 | `write_text` | `summary_path.parent / 'figure-caption-corpus.json'` | script-only |
| `scripts/m058_plotextractor_similarity.py` | 152 | `write_text` | `edges_path` | script-only |
| `scripts/m058_plotextractor_similarity.py` | 155 | `write_text` | `summary_path` | script-only |
| `scripts/m059_build_manifest.py` | 179 | `write_text` | `actual_output` | script-only |
| `scripts/m059_e2e_test.py` | 30 | `write_text` | `path` | script-only |
| `scripts/m060b_graph_stats.py` | 244 | `write_text` | `json_path` | script-only |
| `scripts/m060b_graph_stats.py` | 245 | `write_text` | `md_path` | script-only |
| `scripts/m060b_graph_validate.py` | 291 | `write_text` | `json_path` | script-only |
| `scripts/m060b_graph_validate.py` | 292 | `write_text` | `md_path` | script-only |
| `scripts/m060b_graph_visualize.py` | 171 | `write_bytes` | `output_path` | script-only |
| `scripts/m060b_two_hop_preview.py` | 111 | `write_text` | `output_path` | script-only |
| `scripts/m060c_applicability_matrix.py` | 472 | `write_text` | `json_path` | script-only |
| `scripts/m060c_applicability_matrix.py` | 473 | `write_text` | `markdown_path` | script-only |
| `scripts/m060c_benchmark.py` | 396 | `write_text` | `json_path` | script-only |
| `scripts/m060c_benchmark.py` | 397 | `write_text` | `md_path` | script-only |
| `scripts/m060g_figure_judge.py` | 722 | `write_text` | `output_dir / 'comparison.json'` | script-only |
| `scripts/m060g_figure_judge.py` | 725 | `write_text` | `output_dir / 'judge-summary.json'` | script-only |
| `scripts/m060g_figure_judge.py` | 794 | `write_text` | `path` | script-only |
| `scripts/m060g_smoke_test.py` | 353 | `write_text` | `output_path` | script-only |
| `scripts/m061_anchor_pilot.py` | 137 | `write_text` | `path` | script-only |
| `scripts/m061_anchor_pilot.py` | 511 | `write_bytes` | `pdf_path` | script-only |
| `scripts/m061_anchor_pilot.py` | 526 | `write_bytes` | `source_path` | script-only |
| `scripts/m061_anchor_pilot.py` | 739 | `write_text` | `tei_path` | script-only |
| `scripts/m061_anchor_pilot.py` | 846 | `write_text` | `parser_dir / 'opendataloader.md'` | script-only |
| `scripts/m061_anchor_pilot.py` | 1358 | `write_text` | `decision_path` | script-only |
| `scripts/m061_full_5_anchors.py` | 65 | `write_text` | `path` | script-only |
| `scripts/m061_full_5_anchors.py` | 190 | `write_bytes` | `pdf_path` | script-only |
| `scripts/m061_full_5_anchors.py` | 193 | `write_bytes` | `eprint_path` | script-only |
| `scripts/m061_full_5_anchors.py` | 739 | `write_text` | `BASE_OUTPUT_DIR / 's02-decision.md'` | script-only |
| `scripts/m061_full_5_anchors.py` | 761 | `write_text` | `BASE_OUTPUT_DIR / 's02-decision.md'` | script-only |
| `scripts/m061_synthesis.py` | 546 | `write_text` | `path` | script-only |
| `scripts/m063_graphdb_benchmark.py` | 381 | `write_text` | `output` | script-only |
| `scripts/m066_graphdb_full_benchmark.py` | 675 | `write_text` | `report_path` | script-only |
| `scripts/m066_graphdb_full_benchmark.py` | 676 | `write_text` | `artifact_dir / 'scoring-matrix.md'` | script-only |
| `scripts/m066_graphdb_full_benchmark.py` | 683 | `write_text` | `output` | script-only |
| `scripts/m068_integration_test.py` | 368 | `write_text` | `results_path` | script-only |
| `scripts/m068_integration_test.py` | 371 | `write_text` | `report_path` | script-only |
| `scripts/m103_extraction_prototype.py` | 478 | `write_text` | `out` | script-only |
| `scripts/probe_m033_opendataloader_adaptix_adapter.py` | 89 | `write_text` | `path` | script-only |
| `scripts/probe_m033_opendataloader_adaptix_adapter.py` | 301 | `write_text` | `output_dir / 'adaptix-adapter-report.md'` | script-only |
| `scripts/probe_m043_sidecar_runtime_readiness.py` | 38 | `write_text` | `path` | script-only |
| `scripts/probe_m043_sidecar_runtime_readiness.py` | 45 | `write_text` | `path` | script-only |
| `scripts/probe_m053_grobid_pilot.py` | 73 | `write_bytes` | `tmp_path` | script-only |
| `scripts/render_bounded_repair_prototype.py` | 91 | `write_text` | `json_output` | script-only |
| `scripts/render_bounded_repair_prototype.py` | 92 | `write_text` | `markdown_output` | script-only |
| `scripts/render_chunk_repair_contract.py` | 85 | `write_text` | `json_output` | script-only |
| `scripts/render_chunk_repair_contract.py` | 86 | `write_text` | `markdown_output` | script-only |
| `scripts/render_m055_report.py` | 53 | `write_text` | `path` | script-only |
| `scripts/render_m055deep_report.py` | 408 | `write_text` | `output_path` | script-only |
| `scripts/render_m056_report.py` | 492 | `write_text` | `output` | script-only |
| `scripts/render_reviewer_packet_prototype.py` | 267 | `write_text` | `temp_path` | script-only |
| `scripts/repair_m042_linked_metadata.py` | 63 | `write_text` | `path` | script-only |
| `scripts/repair_m042_linked_metadata.py` | 70 | `write_text` | `path` | script-only |
| `scripts/replay_m025_article_loader.py` | 27 | `write_text` | `path` | script-only |
| `scripts/replay_m027_current_pipeline_baseline.py` | 119 | `write_text` | `path` | script-only |
| `scripts/replay_m027_current_pipeline_baseline.py` | 124 | `write_text` | `path` | script-only |
| `scripts/replay_m027_current_pipeline_baseline.py` | 737 | `write_text` | `path` | script-only |
| `scripts/replay_m027_end_to_end_mixed_replay.py` | 140 | `write_text` | `path` | script-only |
| `scripts/replay_m027_end_to_end_mixed_replay.py` | 147 | `write_text` | `path` | script-only |
| `scripts/replay_m027_end_to_end_mixed_replay.py` | 1038 | `write_text` | `path` | script-only |
| `scripts/replay_m028_smoke_closeout.py` | 882 | `write_text` | `events_path` | script-only |
| `scripts/replay_m028_smoke_closeout.py` | 886 | `write_text` | `summary_path` | script-only |
| `scripts/replay_m028_smoke_closeout.py` | 887 | `write_text` | `report_path` | script-only |
| `scripts/replay_m031_import_boundary_rehearsal.py` | 167 | `write_text` | `summary_path_out` | script-only |
| `scripts/replay_m031_import_boundary_rehearsal.py` | 170 | `write_text` | `diagnostics_path_out` | script-only |
| `scripts/replay_m031_import_boundary_rehearsal.py` | 177 | `write_text` | `report_path_out` | script-only |
| `scripts/run_m029_unified_loader_runtime_smoke.py` | 56 | `open` | `fd` | script-only |
| `scripts/run_m029_unified_replay.py` | 56 | `open` | `fd` | script-only |
| `scripts/run_m044_live_grobid_candidate_probe.py` | 55 | `write_text` | `path` | script-only |
| `scripts/run_m044_live_grobid_candidate_probe.py` | 62 | `write_text` | `path` | script-only |
| `scripts/run_m122_mutation_smoke.py` | 104 | `write_text` | `spec.path` | script-only |
| `scripts/run_m122_mutation_smoke.py` | 113 | `write_text` | `spec.path` | script-only |
| `scripts/run_pipeline_architecture_acceptance.py` | 174 | `write_text` | `summary_path` | script-only |
| `scripts/select_m041_mixed_connectivity_batch.py` | 97 | `write_text` | `path` | script-only |
| `scripts/select_m041_mixed_connectivity_batch.py` | 104 | `write_text` | `path` | script-only |
| `scripts/soak_universal_kb_queue.py` | 274 | `write_text` | `args.json_out` | script-only |
| `scripts/sync_codebase_memory_governance.py` | 516 | `write_text` | `output` | script-only |
| `scripts/sync_codebase_memory_governance.py` | 517 | `write_text` | `graph_output` | script-only |
| `scripts/synthesize_m027_pipeline_readiness.py` | 1248 | `write_text` | `path` | script-only |
| `scripts/synthesize_m027_pipeline_readiness.py` | 1253 | `write_text` | `path` | script-only |
| `scripts/synthesize_m027_pipeline_readiness.py` | 1377 | `write_text` | `path` | script-only |
| `scripts/synthesize_m029_unified_readiness.py` | 59 | `open` | `fd` | script-only |
| `scripts/test_fd_contract.py` | 1418 | `write_text` | `artifact_dir / RESULTS_JSON` | script-only |
| `scripts/test_fd_contract.py` | 1533 | `write_text` | `artifact_dir / REPORT_MD` | script-only |
| `scripts/test_fd_contract.py` | 1569 | `write_text` | `artifact_dir / GAP_MD` | script-only |
| `scripts/update_m043_target_subset_post_m053.py` | 144 | `write_text` | `output_path` | script-only |
| `scripts/update_m043_target_subset_post_m054.py` | 127 | `write_text` | `DEFAULT_TARGET_PATH` | script-only |
| `scripts/verify_article_catalog.py` | 73 | `write_text` | `selection_path` | script-only |
| `scripts/verify_m022_final_gate.py` | 387 | `write_text` | `output_path` | script-only |
| `scripts/verify_m023_artifact_scaffold_gate.py` | 484 | `write_text` | `path` | script-only |
| `scripts/verify_m023_artifact_scaffold_gate.py` | 489 | `write_text` | `path` | script-only |
| `scripts/verify_m025_article_catalog.py` | 1550 | `write_text` | `args.write_report` | script-only |
| `scripts/verify_m025_baseline_recovery_outputs.py` | 447 | `write_text` | `args.write_summary` | script-only |
| `scripts/verify_m025_baseline_recovery_replay.py` | 128 | `write_text` | `path` | script-only |
| `scripts/verify_m025_baseline_recovery_replay.py` | 494 | `write_text` | `path` | script-only |
| `scripts/verify_m025_baseline_recovery_replay.py` | 520 | `write_text` | `args.write_events` | script-only |
| `scripts/verify_m025_boundary_replay_completion.py` | 89 | `write_text` | `path` | script-only |
| `scripts/verify_m025_boundary_replay_completion.py` | 941 | `write_text` | `path` | script-only |
| `scripts/verify_m025_boundary_replay_completion.py` | 999 | `write_text` | `args.write_events` | script-only |
| `scripts/verify_m025_evidence_boundaries.py` | 356 | `write_text` | `path` | script-only |
| `scripts/verify_m025_evidence_boundaries.py` | 803 | `write_text` | `path` | script-only |
| `scripts/verify_m025_evidence_boundaries.py` | 831 | `write_text` | `args.write_events` | script-only |
| `scripts/verify_m025_final_preprocessing_replay.py` | 109 | `write_text` | `path` | script-only |
| `scripts/verify_m025_final_preprocessing_replay.py` | 562 | `write_text` | `path` | script-only |
| `scripts/verify_m025_final_preprocessing_replay.py` | 615 | `write_text` | `args.write_events` | script-only |
| `scripts/verify_m027_end_to_end_mixed_replay.py` | 1307 | `write_text` | `verification_path` | script-only |
| `scripts/verify_m027_end_to_end_mixed_replay.py` | 1343 | `write_text` | `report_path` | script-only |
| `scripts/verify_m027_mixed_source_catalog.py` | 373 | `write_text` | `REPORT_PATH` | script-only |
| `scripts/verify_m027_provenance_and_riskratchet_gate.py` | 776 | `write_text` | `path` | script-only |
| `scripts/verify_m027_provenance_and_riskratchet_gate.py` | 781 | `write_text` | `path` | script-only |
| `scripts/verify_m027_provenance_and_riskratchet_gate.py` | 853 | `write_text` | `path` | script-only |
| `scripts/verify_m027_source_acquisition_boundary.py` | 117 | `write_text` | `path` | script-only |
| `scripts/verify_m027_source_acquisition_boundary.py` | 122 | `write_text` | `path` | script-only |
| `scripts/verify_m027_source_acquisition_boundary.py` | 832 | `write_text` | `args.report` | script-only |
| `scripts/verify_m029_post_validation_remediation.py` | 1205 | `write_text` | `path` | script-only |
| `scripts/verify_m029_unified_conversion_quality_boundary.py` | 114 | `open` | `fd` | script-only |
| `scripts/verify_m029_unified_loader_runtime_smoke.py` | 68 | `open` | `fd` | script-only |
| `scripts/verify_m029_unified_readiness.py` | 506 | `write_text` | `path` | script-only |
| `scripts/verify_m029_unified_source_acquisition.py` | 55 | `write_text` | `path` | script-only |
| `scripts/verify_m029_unified_source_acquisition.py` | 60 | `write_text` | `path` | script-only |
| `scripts/verify_m029_unified_source_acquisition.py` | 795 | `write_text` | `args.write_report` | script-only |
| `scripts/verify_m029_unified_source_acquisition.py` | 819 | `write_text` | `args.write_report` | script-only |
| `scripts/verify_m029_validation_remediation.py` | 989 | `write_text` | `path` | script-only |
| `scripts/verify_m031_chunk_evidence_replay.py` | 115 | `write_text` | `path` | script-only |
| `scripts/verify_m031_parser_conversion_replay.py` | 107 | `write_text` | `path` | script-only |
| `scripts/verify_m031_process_continuity_audit.py` | 246 | `write_text` | `path` | script-only |
| `scripts/verify_m031_process_continuity_audit.py` | 251 | `write_text` | `path` | script-only |
| `scripts/verify_m031_s05_closeout.py` | 253 | `write_text` | `path` | script-only |
| `scripts/verify_m031_s05_closeout.py` | 258 | `write_text` | `path` | script-only |
| `scripts/verify_m031_s05_closeout.py` | 266 | `write_text` | `path` | script-only |
| `scripts/verify_m031_validation_remediation.py` | 1045 | `write_text` | `path` | script-only |
| `scripts/verify_m031_validation_remediation.py` | 1050 | `write_text` | `path` | script-only |
| `scripts/verify_m031_validation_remediation.py` | 1057 | `write_text` | `path` | script-only |
| `scripts/verify_m033_combined_parser_architecture.py` | 166 | `write_text` | `path` | script-only |
| `scripts/verify_m033_combined_parser_architecture.py` | 196 | `write_text` | `path` | script-only |
| `scripts/verify_m033_external_parser_quality_plan.py` | 190 | `write_text` | `path` | script-only |
| `scripts/verify_m033_external_parser_quality_plan.py` | 220 | `write_text` | `path` | script-only |
| `scripts/verify_m033_grobid_probe.py` | 177 | `write_text` | `path` | script-only |
| `scripts/verify_m033_grobid_probe.py` | 208 | `write_text` | `path` | script-only |
| `scripts/verify_m033_opendataloader_adaptix_adapter.py` | 42 | `write_text` | `path` | script-only |
| `scripts/verify_m033_opendataloader_adaptix_adapter.py` | 173 | `write_text` | `adapter_dir / 'adaptix-adapter-closeout-report.md'` | script-only |
| `scripts/verify_m033_quantmind_pattern_study.py` | 216 | `write_text` | `path` | script-only |
| `scripts/verify_m033_quantmind_pattern_study.py` | 249 | `write_text` | `path` | script-only |
| `scripts/verify_m072_queue_benchmark_gate.py` | 97 | `write_text` | `output_path` | script-only |
| `scripts/verify_m073_queue_evidence_gate.py` | 87 | `write_text` | `output_path` | script-only |
| `scripts/verify_test_architecture.py` | 188 | `write_text` | `json_path` | script-only |
| `scripts/verify_test_architecture.py` | 189 | `write_text` | `markdown_path` | script-only |
