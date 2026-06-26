# M179 Residual Script Candidates

## Baseline

```text
script-only=170
exact_paths=108
families=73
```

## Families

| Family | Count | Exact source paths |
|---|---:|---|
| `m057` | 15 | `scripts/m057_compare_marker_opendataloader.py` (2)<br>`scripts/m057_figure_similarity.py` (2)<br>`scripts/m057_marker_extract_5.py` (2)<br>`scripts/m057_table_similarity.py` (2)<br>`scripts/legacy/m057_table_embed.py` (1)<br>`scripts/m057_build_graph_manifest.py` (1)<br>`scripts/m057_compare_marker_opendataloader_1pdf.py` (1)<br>`scripts/m057_fd_validate.py` (1)<br>`scripts/m057_figure_caption_build.py` (1)<br>`scripts/m057_figure_embed.py` (1)<br>`scripts/m057_table_text_build.py` (1) |
| `m060` | 13 | `scripts/m060g_figure_judge.py` (3)<br>`scripts/m060b_graph_stats.py` (2)<br>`scripts/m060b_graph_validate.py` (2)<br>`scripts/m060c_applicability_matrix.py` (2)<br>`scripts/m060c_benchmark.py` (2)<br>`scripts/m060b_graph_visualize.py` (1)<br>`scripts/m060b_two_hop_preview.py` (1) |
| `verify_m031` | 10 | `scripts/verify_m031_s05_closeout.py` (3)<br>`scripts/verify_m031_validation_remediation.py` (3)<br>`scripts/verify_m031_process_continuity_audit.py` (2)<br>`scripts/verify_m031_chunk_evidence_replay.py` (1)<br>`scripts/verify_m031_parser_conversion_replay.py` (1) |
| `verify_m033` | 10 | `scripts/verify_m033_combined_parser_architecture.py` (2)<br>`scripts/verify_m033_external_parser_quality_plan.py` (2)<br>`scripts/verify_m033_grobid_probe.py` (2)<br>`scripts/verify_m033_opendataloader_adaptix_adapter.py` (2)<br>`scripts/verify_m033_quantmind_pattern_study.py` (2) |
| `verify_m029` | 8 | `scripts/verify_m029_unified_source_acquisition.py` (4)<br>`scripts/verify_m029_post_validation_remediation.py` (1)<br>`scripts/verify_m029_unified_conversion_quality_boundary.py` (1)<br>`scripts/verify_m029_unified_readiness.py` (1)<br>`scripts/verify_m029_validation_remediation.py` (1) |
| `m058` | 6 | `scripts/m058_marker_extract_5.py` (2)<br>`scripts/m058_plotextractor_similarity.py` (2)<br>`scripts/m058_build_graph_manifest.py` (1)<br>`scripts/m058_plotextractor_embed.py` (1) |
| `build_m028` | 4 | `scripts/build_m028_hermes_digest_projection.py` (2)<br>`scripts/build_m028_source_metadata_adapters.py` (2) |
| `verify_m027` | 4 | `scripts/verify_m027_source_acquisition_boundary.py` (3)<br>`scripts/verify_m027_mixed_source_catalog.py` (1) |
| `audit_test_architecture` | 3 | `scripts/audit_test_architecture.py` (3) |
| `m066` | 3 | `scripts/m066_graphdb_full_benchmark.py` (3) |
| `replay_m031` | 3 | `scripts/replay_m031_import_boundary_rehearsal.py` (3) |
| `test_fd_contract` | 3 | `scripts/test_fd_contract.py` (3) |
| `acquire_linked_target_pdfs` | 2 | `scripts/acquire_linked_target_pdfs.py` (2) |
| `acquire_m056_wave` | 2 | `scripts/acquire_m056_wave.py` (2) |
| `analyze_m056_wave_1` | 2 | `scripts/analyze_m056_wave_1.py` (2) |
| `analyze_m056_wave_2` | 2 | `scripts/analyze_m056_wave_2.py` (2) |
| `analyze_m056_wave_3` | 2 | `scripts/analyze_m056_wave_3.py` (2) |
| `analyze_m056_wave_4` | 2 | `scripts/analyze_m056_wave_4.py` (2) |
| `analyze_m056_wave_5` | 2 | `scripts/analyze_m056_wave_5.py` (2) |
| `audit_locator_evidence` | 2 | `scripts/audit_locator_evidence.py` (2) |
| `audit_m042` | 2 | `scripts/audit_m042_connectivity_groups.py` (2) |
| `augment_m073_evidence_paths` | 2 | `scripts/augment_m073_evidence_paths.py` (2) |
| `build_m043` | 2 | `scripts/build_m043_sidecar_packets.py` (2) |
| `capture_m027_mixed_source_sources` | 2 | `scripts/capture_m027_mixed_source_sources.py` (2) |
| `check_project_trajectory` | 2 | `scripts/check_project_trajectory.py` (2) |
| `m052` | 2 | `scripts/m052_rlm_e2e.py` (2) |
| `m059` | 2 | `scripts/m059_build_manifest.py` (1)<br>`scripts/m059_e2e_test.py` (1) |
| `m068` | 2 | `scripts/m068_integration_test.py` (2) |
| `probe_m033_opendataloader_adaptix_adapter` | 2 | `scripts/probe_m033_opendataloader_adaptix_adapter.py` (2) |
| `probe_m043_sidecar_runtime_readiness` | 2 | `scripts/probe_m043_sidecar_runtime_readiness.py` (2) |
| `render_bounded_repair_prototype` | 2 | `scripts/render_bounded_repair_prototype.py` (2) |
| `render_chunk_repair_contract` | 2 | `scripts/render_chunk_repair_contract.py` (2) |
| `repair_m042_linked_metadata` | 2 | `scripts/repair_m042_linked_metadata.py` (2) |
| `run_m044_live_grobid_candidate_probe` | 2 | `scripts/run_m044_live_grobid_candidate_probe.py` (2) |
| `select_m041_mixed_connectivity_batch` | 2 | `scripts/select_m041_mixed_connectivity_batch.py` (2) |
| `sync_codebase_memory_governance` | 2 | `scripts/sync_codebase_memory_governance.py` (2) |
| `verify_m023` | 2 | `scripts/verify_m023_artifact_scaffold_gate.py` (2) |
| `verify_m025` | 2 | `scripts/verify_m025_article_catalog.py` (1)<br>`scripts/verify_m025_baseline_recovery_outputs.py` (1) |
| `verify_test_architecture` | 2 | `scripts/verify_test_architecture.py` (2) |
| `analyze_m056_wave_6` | 1 | `scripts/analyze_m056_wave_6.py` (1) |
| `audit_m053` | 1 | `scripts/audit_m053_grobid_pilot.py` (1) |
| `audit_m054` | 1 | `scripts/audit_m054_pdf_acquisition.py` (1) |
| `audit_pipeline_scripts` | 1 | `scripts/audit_pipeline_scripts.py` (1) |
| `benchmark_m055_availability_probe` | 1 | `scripts/benchmark_m055_availability_probe.py` (1) |
| `benchmark_m055_corpus_manifest` | 1 | `scripts/benchmark_m055_corpus_manifest.py` (1) |
| `benchmark_m055_grobid_only` | 1 | `scripts/benchmark_m055_grobid_only.py` (1) |
| `benchmark_m055_hybrid_routing` | 1 | `scripts/benchmark_m055_hybrid_routing.py` (1) |
| `benchmark_m055_opendataloader_only` | 1 | `scripts/benchmark_m055_opendataloader_only.py` (1) |
| `benchmark_m055_vendor_check` | 1 | `scripts/benchmark_m055_vendor_check.py` (1) |
| `benchmark_m055deep_grobid_fulltext` | 1 | `scripts/benchmark_m055deep_grobid_fulltext.py` (1) |
| `benchmark_m055deep_hybrid_routing_20` | 1 | `scripts/benchmark_m055deep_hybrid_routing_20.py` (1) |
| `benchmark_m055deep_opendataloader_correctness` | 1 | `scripts/benchmark_m055deep_opendataloader_correctness.py` (1) |
| `build_m055` | 1 | `scripts/build_m055deep_corpus_manifest_20.py` (1) |
| `compare_m055_header_vs_fulltext` | 1 | `scripts/compare_m055_header_vs_fulltext.py` (1) |
| `convert_m027` | 1 | `scripts/convert_m027_source_quality_boundary.py` (1) |
| `convert_m029` | 1 | `scripts/convert_m029_unified_source_quality_boundary.py` (1) |
| `emit_m056_candidate_edges` | 1 | `scripts/emit_m056_candidate_edges.py` (1) |
| `m061` | 1 | `scripts/m061_synthesis.py` (1) |
| `m063` | 1 | `scripts/m063_graphdb_benchmark.py` (1) |
| `m103` | 1 | `scripts/m103_extraction_prototype.py` (1) |
| `probe_m053_grobid_pilot` | 1 | `scripts/probe_m053_grobid_pilot.py` (1) |
| `render_m055_report` | 1 | `scripts/render_m055_report.py` (1) |
| `render_m055deep_report` | 1 | `scripts/render_m055deep_report.py` (1) |
| `render_m056_report` | 1 | `scripts/render_m056_report.py` (1) |
| `render_reviewer_packet_prototype` | 1 | `scripts/render_reviewer_packet_prototype.py` (1) |
| `replay_m025` | 1 | `scripts/replay_m025_article_loader.py` (1) |
| `run_m029_unified_replay` | 1 | `scripts/run_m029_unified_replay.py` (1) |
| `run_pipeline_architecture_acceptance` | 1 | `scripts/run_pipeline_architecture_acceptance.py` (1) |
| `synthesize_m029_unified_readiness` | 1 | `scripts/synthesize_m029_unified_readiness.py` (1) |
| `update_m043_target_subset_post_m053` | 1 | `scripts/update_m043_target_subset_post_m053.py` (1) |
| `update_m043_target_subset_post_m054` | 1 | `scripts/update_m043_target_subset_post_m054.py` (1) |
| `verify_article_catalog` | 1 | `scripts/verify_article_catalog.py` (1) |
| `verify_m022` | 1 | `scripts/verify_m022_final_gate.py` (1) |

## Candidate notes

- Use family grouping only for review; implementation must match exact source paths.
- Prefer a family with stable milestone semantics and enough movement to justify tests.
- Reject generic names or target-name based rules.

## Initial recommendation

Select the largest safe exact family with cohesive milestone semantics after reviewing the table above.
