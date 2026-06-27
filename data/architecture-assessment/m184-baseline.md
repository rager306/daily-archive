# M184 Baseline

## Inventory summary

```text
total_records=341
script-only=89
unknown=0
shared-state=0
```

## Residual script-only groups

### acquisition-source: 10

- `scripts/acquire_linked_target_pdfs.py` :: `log_path`
- `scripts/acquire_linked_target_pdfs.py` :: `tmp_path`
- `scripts/acquire_m056_wave.py` :: `tmp_path`
- `scripts/acquire_m056_wave.py` :: `tmp_path`
- `scripts/audit_m054_pdf_acquisition.py` :: `DEFAULT_AUDIT_PATH`
- `scripts/capture_m027_mixed_source_sources.py` :: `report_path`
- `scripts/capture_m027_mixed_source_sources.py` :: `report_path`
- `scripts/convert_m027_source_quality_boundary.py` :: `fd`
- `scripts/convert_m029_unified_source_quality_boundary.py` :: `fd`
- `scripts/emit_m056_candidate_edges.py` :: `output`

### audit-analysis: 24

- `scripts/analyze_m056_wave_1.py` :: `tmp_path`
- `scripts/analyze_m056_wave_1.py` :: `tmp_path`
- `scripts/analyze_m056_wave_2.py` :: `tmp_path`
- `scripts/analyze_m056_wave_2.py` :: `tmp_path`
- `scripts/analyze_m056_wave_3.py` :: `tmp_path`
- `scripts/analyze_m056_wave_3.py` :: `tmp_path`
- `scripts/analyze_m056_wave_4.py` :: `tmp_path`
- `scripts/analyze_m056_wave_4.py` :: `tmp_path`
- `scripts/analyze_m056_wave_5.py` :: `tmp_path`
- `scripts/analyze_m056_wave_5.py` :: `tmp_path`
- `scripts/analyze_m056_wave_6.py` :: `tmp`
- `scripts/audit_locator_evidence.py` :: `destination`
- `scripts/audit_locator_evidence.py` :: `destination`
- `scripts/audit_m042_connectivity_groups.py` :: `path`
- `scripts/audit_m042_connectivity_groups.py` :: `path`
- `scripts/audit_m053_grobid_pilot.py` :: `output_path`
- `scripts/audit_pipeline_scripts.py` :: `path`
- `scripts/check_project_trajectory.py` :: `path`
- `scripts/check_project_trajectory.py` :: `path`
- `scripts/test_fd_contract.py` :: `artifact_dir / GAP_MD`
- `scripts/test_fd_contract.py` :: `artifact_dir / REPORT_MD`
- `scripts/test_fd_contract.py` :: `artifact_dir / RESULTS_JSON`
- `scripts/verify_test_architecture.py` :: `json_path`
- `scripts/verify_test_architecture.py` :: `markdown_path`

### experiment-probe: 9

- `scripts/m052_rlm_e2e.py` :: `audit_json_path`
- `scripts/m052_rlm_e2e.py` :: `audit_md_path`
- `scripts/m058_marker_extract_5.py` :: `OUTPUT_ROOT / 'summary.json'`
- `scripts/m058_marker_extract_5.py` :: `PER_PDF_DIR / f'{sample.arxiv_id}.json'`
- `scripts/m058_plotextractor_embed.py` :: `output_path`
- `scripts/m058_plotextractor_similarity.py` :: `edges_path`
- `scripts/m058_plotextractor_similarity.py` :: `summary_path`
- `scripts/m068_integration_test.py` :: `report_path`
- `scripts/m068_integration_test.py` :: `results_path`

### governance-sync: 4

- `scripts/augment_m073_evidence_paths.py` :: `output`
- `scripts/augment_m073_evidence_paths.py` :: `path`
- `scripts/sync_codebase_memory_governance.py` :: `graph_output`
- `scripts/sync_codebase_memory_governance.py` :: `output`

### graph-connectivity-probe: 13

- `scripts/m058_build_graph_manifest.py` :: `path`
- `scripts/m063_graphdb_benchmark.py` :: `output`
- `scripts/probe_m033_opendataloader_adaptix_adapter.py` :: `output_dir / 'adaptix-adapter-report.md'`
- `scripts/probe_m033_opendataloader_adaptix_adapter.py` :: `path`
- `scripts/probe_m043_sidecar_runtime_readiness.py` :: `path`
- `scripts/probe_m043_sidecar_runtime_readiness.py` :: `path`
- `scripts/probe_m053_grobid_pilot.py` :: `tmp_path`
- `scripts/repair_m042_linked_metadata.py` :: `path`
- `scripts/repair_m042_linked_metadata.py` :: `path`
- `scripts/run_m044_live_grobid_candidate_probe.py` :: `path`
- `scripts/run_m044_live_grobid_candidate_probe.py` :: `path`
- `scripts/select_m041_mixed_connectivity_batch.py` :: `path`
- `scripts/select_m041_mixed_connectivity_batch.py` :: `path`

### manifest-cache-index: 3

- `scripts/benchmark_m055_corpus_manifest.py` :: `output_path`
- `scripts/build_m055deep_corpus_manifest_20.py` :: `output_path`
- `scripts/m059_build_manifest.py` :: `actual_output`

### misc: 16

- `scripts/build_m043_sidecar_packets.py` :: `path`
- `scripts/build_m043_sidecar_packets.py` :: `path`
- `scripts/compare_m055_header_vs_fulltext.py` :: `tmp_path`
- `scripts/m059_e2e_test.py` :: `path`
- `scripts/m061_synthesis.py` :: `path`
- `scripts/m103_extraction_prototype.py` :: `out`
- `scripts/run_pipeline_architecture_acceptance.py` :: `summary_path`
- `scripts/synthesize_m029_unified_readiness.py` :: `fd`
- `scripts/update_m043_target_subset_post_m053.py` :: `output_path`
- `scripts/update_m043_target_subset_post_m054.py` :: `DEFAULT_TARGET_PATH`
- `scripts/verify_article_catalog.py` :: `selection_path`
- `scripts/verify_m022_final_gate.py` :: `output_path`
- `scripts/verify_m023_artifact_scaffold_gate.py` :: `path`
- `scripts/verify_m023_artifact_scaffold_gate.py` :: `path`
- `scripts/verify_m025_article_catalog.py` :: `args.write_report`
- `scripts/verify_m025_baseline_recovery_outputs.py` :: `args.write_summary`

### render-report-contract: 8

- `scripts/render_bounded_repair_prototype.py` :: `json_output`
- `scripts/render_bounded_repair_prototype.py` :: `markdown_output`
- `scripts/render_chunk_repair_contract.py` :: `json_output`
- `scripts/render_chunk_repair_contract.py` :: `markdown_output`
- `scripts/render_m055_report.py` :: `path`
- `scripts/render_m055deep_report.py` :: `output_path`
- `scripts/render_m056_report.py` :: `output`
- `scripts/render_reviewer_packet_prototype.py` :: `temp_path`

### replay-conversion: 2

- `scripts/replay_m025_article_loader.py` :: `path`
- `scripts/run_m029_unified_replay.py` :: `fd`

## Guardrails at start

- `unknown=0`
- `shared-state=0`
- Canonical baseline is `data/architecture-assessment/write-path-inventory-canonical.json`.
- M184 starts from `script-only=89`.
