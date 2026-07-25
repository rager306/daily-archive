from __future__ import annotations

import json
from pathlib import Path

from scripts.inventory_write_paths import _classify, render_delta_markdown


def test_reviewed_shared_state_records_get_precise_categories() -> None:
    assert _classify(
        Path("src/research_graph/application/validation/batch_state.py"),
        "write_text",
        "output_path",
        None,
    ) == ("run-owned-state", "workflow-owned batch state replacement")
    assert _classify(
        Path("src/research_graph/infrastructure/corpus/ingestion/catalog_adapters.py"),
        "write_text",
        "summary_path",
        None,
    ) == (
        "legacy-evidence-regeneration",
        "reviewed legacy ingest summary regeneration",
    )
    assert _classify(
        Path("src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py"),
        "write_text",
        "report_path",
        None,
    ) == (
        "legacy-evidence-regeneration",
        "reviewed legacy ingest report regeneration",
    )
    assert _classify(
        Path("src/research_graph/infrastructure/repair/chunk_baseline_measurement.py"),
        "write_text",
        "index_path",
        None,
    ) == ("caller-owned-index", "caller-provided paired review index output")


def test_graph_readiness_outputs_get_precise_category() -> None:
    assert _classify(
        Path("src/research_graph/infrastructure/graph/readiness/export.py"),
        "write_text",
        "summary_path",
        None,
    ) == ("graph-readiness-evidence", "reviewed graph-readiness evidence output")


def test_source_asset_and_article_artifact_outputs_get_precise_categories() -> None:
    assert _classify(
        Path("src/research_graph/infrastructure/papers/source_assets/registry.py"),
        "write_text",
        "output_dir / 'source-asset-summary.json'",
        None,
    ) == ("source-asset-package", "reviewed source asset package output")
    assert _classify(
        Path("src/research_graph/cli/commands/article_artifacts.py"),
        "write_text",
        "diagnostics_path",
        None,
    ) == ("article-artifact-package", "reviewed article artifact package output")


def test_m176_script_wave_one_outputs_get_precise_categories_without_generic_script_rules() -> None:
    assert _classify(
        Path("scripts/m061_anchor_pilot.py"),
        "write_text",
        "path",
        None,
    ) == ("m061-acquisition-pipeline-output", "reviewed M061 acquisition pipeline output")
    assert _classify(
        Path("scripts/m058_plotextractor_extract.py"),
        "write_text",
        "summary_path",
        None,
    ) == (
        "figure-extraction-benchmark-output",
        "reviewed figure extraction benchmark output",
    )
    assert _classify(
        Path("scripts/build_m028_pdf_acquisition_diagnostics.py"),
        "write_text",
        "out_dir / REPORT_FILENAME",
        None,
    ) == ("m028-acquisition-evidence-output", "reviewed M028 acquisition evidence output")
    assert _classify(
        Path("scripts/verify_m029_future_unlisted.py"),
        "write_text",
        "path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m177_r024_script_outputs_get_precise_categories_without_generic_script_rules() -> None:
    assert _classify(
        Path("scripts/build_r024_20_document_corpus_selection.py"),
        "write_text",
        "OUT_SELECTION",
        None,
    ) == ("r024-corpus-selection-output", "reviewed R024 corpus selection output")
    assert _classify(
        Path("scripts/extract_r024_entity_scale_entities.py"),
        "write_text",
        "SUMMARY",
        None,
    ) == ("r024-entity-extraction-output", "reviewed R024 entity extraction output")
    assert _classify(
        Path("scripts/convert_r024_53_pdf_to_text.py"),
        "write_text",
        "out_path",
        None,
    ) == ("r024-conversion-output", "reviewed R024 conversion output")
    assert _classify(
        Path("scripts/build_r024_entity_networkx_probe.py"),
        "write_text",
        "MEMORY_PROFILE",
        None,
    ) == ("r024-networkx-probe-output", "reviewed R024 networkx probe output")
    assert _classify(
        Path("scripts/extract_r024_quality_metrics.py"),
        "write_text",
        "METRICS",
        None,
    ) == ("r024-quality-metrics-output", "reviewed R024 quality metrics output")
    assert _classify(
        Path("scripts/verify_m029_future_unlisted.py"),
        "write_text",
        "path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m177_inventory_report_outputs_get_precise_category() -> None:
    for target in ("args.json", "args.markdown", "args.delta_markdown"):
        assert _classify(
            Path("scripts/inventory_write_paths.py"),
            "write_text",
            target,
            None,
        ) == ("inventory-report-output", "reviewed inventory report output")


def test_m177_markdown_converter_cache_policy_preserves_caller_owned_outputs() -> None:
    for target in ("md_path", "method_path"):
        assert _classify(
            Path("src/research_graph/infrastructure/corpus/sources/markdown_converter.py"),
            "write_text",
            target,
            None,
        ) == ("caller-owned", "caller-provided or adapter-owned output path")


def test_m177_queue_and_smoke_script_outputs_get_precise_categories() -> None:
    assert _classify(
        Path("scripts/soak_universal_kb_queue.py"),
        "write_text",
        "args.json_out",
        None,
    ) == ("queue-soak-output", "reviewed queue soak output")
    assert _classify(
        Path("scripts/verify_m072_queue_benchmark_gate.py"),
        "write_text",
        "output_path",
        None,
    ) == ("queue-gate-output", "reviewed queue gate output")
    assert _classify(
        Path("scripts/replay_m028_smoke_closeout.py"),
        "write_text",
        "summary_path",
        None,
    ) == ("smoke-script-output", "reviewed smoke script output")


def test_m177_universal_kb_workflow_categories_remain_unchanged() -> None:
    assert _classify(
        Path("src/research_graph/workflows/universal_kb/queue.py"),
        "sqlite3.connect",
        "self.db_path",
        None,
    ) == ("database", "database-backed mutable state")
    assert _classify(
        Path("src/research_graph/workflows/universal_kb/smoke.py"),
        "write_text",
        "path",
        None,
    ) == ("caller-owned", "caller-provided or adapter-owned output path")
    assert _classify(
        Path("src/research_graph/workflows/universal_kb/smoke_selection.py"),
        "write_text",
        "args.output",
        None,
    ) == ("run-scoped", "caller/output scoped artifact path")


def test_m178_m027_pipeline_replay_outputs_get_precise_category() -> None:
    for path in (
        "scripts/replay_m027_current_pipeline_baseline.py",
        "scripts/replay_m027_end_to_end_mixed_replay.py",
        "scripts/synthesize_m027_pipeline_readiness.py",
        "scripts/verify_m027_provenance_and_riskratchet_gate.py",
        "scripts/verify_m027_end_to_end_mixed_replay.py",
    ):
        assert _classify(
            Path(path),
            "write_text",
            "path",
            None,
        ) == ("m027-pipeline-replay-output", "reviewed M027 pipeline replay output")
    assert _classify(
        Path("scripts/verify_m031_future_unreviewed.py"),
        "write_text",
        "path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m178_m025_recovery_evidence_outputs_get_precise_category() -> None:
    for path in (
        "scripts/verify_m025_baseline_recovery_replay.py",
        "scripts/verify_m025_boundary_replay_completion.py",
        "scripts/verify_m025_evidence_boundaries.py",
        "scripts/verify_m025_final_preprocessing_replay.py",
        "scripts/capture_m025_article_sources.py",
    ):
        assert _classify(
            Path(path),
            "write_text",
            "path",
            None,
        ) == ("m025-recovery-evidence-output", "reviewed M025 recovery evidence output")
    assert _classify(
        Path("scripts/verify_m033_future_unreviewed.py"),
        "write_text",
        "path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m179_m057_structure_extraction_outputs_get_precise_category() -> None:
    for path in (
        "scripts/m057_compare_marker_opendataloader.py",
        "scripts/m057_figure_similarity.py",
        "scripts/m057_marker_extract_5.py",
        "scripts/m057_table_similarity.py",
        "scripts/legacy/m057_table_embed.py",
        "scripts/m057_build_graph_manifest.py",
        "scripts/m057_compare_marker_opendataloader_1pdf.py",
        "scripts/m057_fd_validate.py",
        "scripts/m057_figure_caption_build.py",
        "scripts/m057_figure_embed.py",
        "scripts/m057_table_text_build.py",
    ):
        assert _classify(
            Path(path),
            "write_text",
            "path",
            None,
        ) == ("m057-structure-extraction-output", "reviewed M057 structure extraction output")
    assert _classify(
        Path("scripts/m057_new_unreviewed_probe.py"),
        "write_text",
        "path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m179_m060_graph_figure_outputs_get_precise_category() -> None:
    for path in (
        "scripts/m060g_figure_judge.py",
        "scripts/m060b_graph_stats.py",
        "scripts/m060b_graph_validate.py",
        "scripts/m060c_applicability_matrix.py",
        "scripts/m060c_benchmark.py",
        "scripts/m060b_graph_visualize.py",
        "scripts/m060b_two_hop_preview.py",
    ):
        assert _classify(
            Path(path),
            "write_text",
            "path",
            None,
        ) == (
            "m060-graph-figure-benchmark-output",
            "reviewed M060 graph and figure benchmark output",
        )
    assert _classify(
        Path("scripts/m060_unreviewed_followup.py"),
        "write_text",
        "path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m180_verify_m031_outputs_get_precise_category() -> None:
    for path in (
        "scripts/verify_m031_s05_closeout.py",
        "scripts/verify_m031_validation_remediation.py",
        "scripts/verify_m031_process_continuity_audit.py",
        "scripts/verify_m031_chunk_evidence_replay.py",
        "scripts/verify_m031_parser_conversion_replay.py",
    ):
        assert _classify(
            Path(path),
            "write_text",
            "path",
            None,
        ) == ("verify-m031-output", "reviewed M031 verification output")
    assert _classify(
        Path("scripts/verify_m031_future_probe.py"),
        "write_text",
        "path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m180_verify_m033_outputs_get_precise_category() -> None:
    for path in (
        "scripts/verify_m033_combined_parser_architecture.py",
        "scripts/verify_m033_external_parser_quality_plan.py",
        "scripts/verify_m033_grobid_probe.py",
        "scripts/verify_m033_opendataloader_adaptix_adapter.py",
        "scripts/verify_m033_quantmind_pattern_study.py",
    ):
        assert _classify(
            Path(path),
            "write_text",
            "path",
            None,
        ) == ("verify-m033-output", "reviewed M033 verification output")
    assert _classify(
        Path("scripts/verify_m033_future_probe.py"),
        "write_text",
        "path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m181_verify_m029_outputs_get_precise_category() -> None:
    for path in (
        "scripts/verify_m029_post_validation_remediation.py",
        "scripts/verify_m029_unified_conversion_quality_boundary.py",
        "scripts/verify_m029_unified_readiness.py",
        "scripts/verify_m029_unified_source_acquisition.py",
        "scripts/verify_m029_validation_remediation.py",
    ):
        assert _classify(
            Path(path),
            "write_text",
            "path",
            None,
        ) == ("verify-m029-output", "reviewed M029 verification output")
    assert _classify(
        Path("scripts/verify_m029_future_unlisted.py"),
        "write_text",
        "path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m181_verify_m027_outputs_get_precise_category() -> None:
    for path in (
        "scripts/verify_m027_mixed_source_catalog.py",
        "scripts/verify_m027_source_acquisition_boundary.py",
    ):
        assert _classify(
            Path(path),
            "write_text",
            "path",
            None,
        ) == ("verify-m027-output", "reviewed M027 verification output")
    assert _classify(
        Path("scripts/verify_m027_future_unlisted.py"),
        "write_text",
        "path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m182_build_m028_outputs_get_precise_category() -> None:
    for path in (
        "scripts/build_m028_hermes_digest_projection.py",
        "scripts/build_m028_source_metadata_adapters.py",
    ):
        assert _classify(
            Path(path),
            "write_text",
            "summary_path",
            None,
        ) == ("build-m028-output", "reviewed M028 builder output")
    assert _classify(
        Path("scripts/build_m028_future_unlisted.py"),
        "write_text",
        "summary_path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m182_replay_m031_outputs_get_precise_category() -> None:
    assert _classify(
        Path("scripts/replay_m031_import_boundary_rehearsal.py"),
        "write_text",
        "summary_path_out",
        None,
    ) == ("replay-m031-output", "reviewed M031 replay output")
    assert _classify(
        Path("scripts/replay_m031_future_unlisted.py"),
        "write_text",
        "summary_path_out",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m183_benchmark_m055_outputs_get_precise_category_without_manifest_movement() -> None:
    for path in (
        "scripts/benchmark_m055_availability_probe.py",
        "scripts/benchmark_m055_grobid_only.py",
        "scripts/benchmark_m055_hybrid_routing.py",
        "scripts/benchmark_m055_opendataloader_only.py",
        "scripts/benchmark_m055_vendor_check.py",
    ):
        assert _classify(
            Path(path),
            "write_text",
            "output_path",
            None,
        ) == ("benchmark-m055-output", "reviewed M055 benchmark output")
    assert _classify(
        Path("scripts/benchmark_m055_corpus_manifest.py"),
        "write_text",
        "output_path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")
    assert _classify(
        Path("scripts/benchmark_m055_future_unlisted.py"),
        "write_text",
        "output_path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m183_benchmark_m055deep_outputs_get_precise_category() -> None:
    for path in (
        "scripts/benchmark_m055deep_grobid_fulltext.py",
        "scripts/benchmark_m055deep_hybrid_routing_20.py",
        "scripts/benchmark_m055deep_opendataloader_correctness.py",
    ):
        assert _classify(
            Path(path),
            "write_text",
            "tmp_path",
            None,
        ) == ("benchmark-m055deep-output", "reviewed M055 deep benchmark output")
    assert _classify(
        Path("scripts/benchmark_m055deep_future_unlisted.py"),
        "write_text",
        "tmp_path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m183_m066_and_test_architecture_audit_outputs_get_precise_categories() -> None:
    assert _classify(
        Path("scripts/m066_graphdb_full_benchmark.py"),
        "write_text",
        "report_path",
        None,
    ) == ("m066-graphdb-benchmark-output", "reviewed M066 graphdb benchmark output")
    for path in (
        "scripts/audit_test_architecture.py",
        "src/research_graph/application/test_architecture_inventory.py",
    ):
        assert _classify(
            Path(path),
            "write_text",
            "markdown_path",
            None,
        ) == ("test-architecture-audit-output", "reviewed test architecture audit output")
    assert _classify(
        Path("scripts/m066_graphdb_future_unlisted.py"),
        "write_text",
        "report_path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")
    assert _classify(
        Path("scripts/audit_test_future_unlisted.py"),
        "write_text",
        "markdown_path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m184_source_acquisition_outputs_get_precise_category() -> None:
    for path, target in (
        ("scripts/acquire_linked_target_pdfs.py", "log_path"),
        ("scripts/acquire_linked_target_pdfs.py", "tmp_path"),
        ("scripts/acquire_m056_wave.py", "tmp_path"),
        ("scripts/audit_m054_pdf_acquisition.py", "DEFAULT_AUDIT_PATH"),
        ("scripts/capture_m027_mixed_source_sources.py", "report_path"),
        ("scripts/convert_m027_source_quality_boundary.py", "fd"),
        ("scripts/convert_m029_unified_source_quality_boundary.py", "fd"),
        ("scripts/emit_m056_candidate_edges.py", "output"),
    ):
        assert _classify(
            Path(path),
            "write_text",
            target,
            None,
        ) == (
            "source-acquisition-evidence-output",
            "reviewed source acquisition evidence output",
        )
    assert _classify(
        Path("scripts/acquire_future_unlisted.py"),
        "write_text",
        "log_path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m184_audit_analysis_outputs_get_precise_category() -> None:
    for path, target in (
        ("scripts/analyze_m056_wave_1.py", "tmp_path"),
        ("scripts/analyze_m056_wave_2.py", "tmp_path"),
        ("scripts/analyze_m056_wave_3.py", "tmp_path"),
        ("scripts/analyze_m056_wave_4.py", "tmp_path"),
        ("scripts/analyze_m056_wave_5.py", "tmp_path"),
        ("scripts/analyze_m056_wave_6.py", "tmp"),
        ("scripts/audit_locator_evidence.py", "destination"),
        ("scripts/audit_m042_connectivity_groups.py", "path"),
        ("scripts/audit_m053_grobid_pilot.py", "output_path"),
        ("scripts/audit_pipeline_scripts.py", "path"),
        ("scripts/check_project_trajectory.py", "path"),
        ("scripts/test_fd_contract.py", "artifact_dir / REPORT_MD"),
        ("scripts/verify_test_architecture.py", "json_path"),
    ):
        assert _classify(
            Path(path),
            "write_text",
            target,
            None,
        ) == ("audit-analysis-output", "reviewed audit analysis output")
    assert _classify(
        Path("scripts/audit_future_unlisted.py"),
        "write_text",
        "path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m184_render_report_contract_outputs_get_precise_category() -> None:
    for path, target in (
        ("scripts/render_bounded_repair_prototype.py", "json_output"),
        ("scripts/render_bounded_repair_prototype.py", "markdown_output"),
        ("scripts/render_chunk_repair_contract.py", "json_output"),
        ("scripts/render_chunk_repair_contract.py", "markdown_output"),
        ("scripts/render_m055_report.py", "path"),
        ("scripts/render_m055deep_report.py", "output_path"),
        ("scripts/render_m056_report.py", "output"),
        ("scripts/render_reviewer_packet_prototype.py", "temp_path"),
    ):
        assert _classify(
            Path(path),
            "write_text",
            target,
            None,
        ) == (
            "render-report-contract-output",
            "reviewed render report contract output",
        )
    assert _classify(
        Path("scripts/render_future_unlisted.py"),
        "write_text",
        "output",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m184_replay_conversion_outputs_get_precise_category() -> None:
    for path, target in (
        ("scripts/replay_m025_article_loader.py", "path"),
        ("scripts/run_m029_unified_replay.py", "fd"),
    ):
        assert _classify(
            Path(path),
            "write_text",
            target,
            None,
        ) == ("replay-conversion-output", "reviewed replay conversion output")
    assert _classify(
        Path("scripts/replay_future_unlisted.py"),
        "write_text",
        "path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m184_graph_connectivity_probe_outputs_get_precise_category_without_manifest() -> None:
    for path, target in (
        ("scripts/m063_graphdb_benchmark.py", "output"),
        ("scripts/probe_m033_opendataloader_adaptix_adapter.py", "path"),
        ("scripts/probe_m033_opendataloader_adaptix_adapter.py", "output_dir / 'adaptix-adapter-report.md'"),
        ("scripts/probe_m043_sidecar_runtime_readiness.py", "path"),
        ("scripts/probe_m053_grobid_pilot.py", "tmp_path"),
        ("scripts/repair_m042_linked_metadata.py", "path"),
        ("scripts/run_m044_live_grobid_candidate_probe.py", "path"),
        ("scripts/select_m041_mixed_connectivity_batch.py", "path"),
    ):
        assert _classify(
            Path(path),
            "write_text",
            target,
            None,
        ) == (
            "graph-connectivity-probe-output",
            "reviewed graph connectivity probe output",
        )
    assert _classify(
        Path("scripts/m058_build_graph_manifest.py"),
        "write_text",
        "path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")
    assert _classify(
        Path("scripts/probe_future_unlisted.py"),
        "write_text",
        "path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_m184_remaining_residual_outputs_get_precise_categories_without_manifests() -> None:
    for path, target, category, reason in (
        ("scripts/augment_m073_evidence_paths.py", "output", "governance-sync-output", "reviewed governance sync output"),
        ("scripts/sync_codebase_memory_governance.py", "graph_output", "governance-sync-output", "reviewed governance sync output"),
        ("scripts/compare_m055_header_vs_fulltext.py", "tmp_path", "experiment-probe-output", "reviewed experiment probe output"),
        ("scripts/m052_rlm_e2e.py", "audit_json_path", "experiment-probe-output", "reviewed experiment probe output"),
        ("scripts/m058_marker_extract_5.py", "OUTPUT_ROOT / 'summary.json'", "experiment-probe-output", "reviewed experiment probe output"),
        ("scripts/m058_plotextractor_similarity.py", "summary_path", "experiment-probe-output", "reviewed experiment probe output"),
        ("scripts/m068_integration_test.py", "report_path", "experiment-probe-output", "reviewed experiment probe output"),
        ("scripts/build_m043_sidecar_packets.py", "path", "misc-architecture-artifact-output", "reviewed misc architecture artifact output"),
        ("scripts/m061_synthesis.py", "path", "misc-architecture-artifact-output", "reviewed misc architecture artifact output"),
        ("scripts/run_pipeline_architecture_acceptance.py", "summary_path", "misc-architecture-artifact-output", "reviewed misc architecture artifact output"),
        ("scripts/update_m043_target_subset_post_m054.py", "DEFAULT_TARGET_PATH", "misc-architecture-artifact-output", "reviewed misc architecture artifact output"),
        ("scripts/verify_m023_artifact_scaffold_gate.py", "path", "misc-architecture-artifact-output", "reviewed misc architecture artifact output"),
        ("scripts/verify_m025_article_catalog.py", "args.write_report", "misc-architecture-artifact-output", "reviewed misc architecture artifact output"),
    ):
        assert _classify(Path(path), "write_text", target, None) == (category, reason)
    for path, target in (
        ("scripts/benchmark_m055_corpus_manifest.py", "output_path"),
        ("scripts/build_m055deep_corpus_manifest_20.py", "output_path"),
        ("scripts/m058_build_graph_manifest.py", "path"),
        ("scripts/m059_build_manifest.py", "actual_output"),
    ):
        assert _classify(Path(path), "write_text", target, None) == (
            "script-only",
            "write occurs in process-boundary script",
        )


def test_daily_cli_outputs_get_precise_category_without_moving_temp_path() -> None:
    assert _classify(
        Path("src/research_graph/cli/__init__.py"),
        "write_text",
        "filepath",
        None,
    ) == ("daily-cli-output", "reviewed daily CLI output")
    assert _classify(
        Path("src/research_graph/cli/__init__.py"),
        "write_text",
        "day_dir / 'papers.json'",
        None,
    ) == ("daily-cli-output", "reviewed daily CLI output")
    assert _classify(
        Path("src/research_graph/cli/__init__.py"),
        "write_text",
        "temp_path",
        None,
    ) == ("temporary", "same-directory temporary write before final replacement")
    assert _classify(
        Path("src/research_graph/infrastructure/example_writer.py"),
        "write_text",
        "filepath",
        None,
    ) == ("caller-owned", "caller-provided or adapter-owned output path")


def test_parser_replay_outputs_get_precise_category() -> None:
    assert _classify(
        Path("src/research_graph/infrastructure/corpus/parsing/replay_adapters.py"),
        "write_text",
        "cache_path",
        None,
    ) == ("parser-replay-output", "reviewed parser replay output")


def test_source_scan_outputs_get_precise_category() -> None:
    assert _classify(
        Path("src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py"),
        "write_text",
        "destination",
        None,
    ) == ("source-scan-output", "reviewed source scan output")


def test_graph_probe_outputs_get_precise_category() -> None:
    assert _classify(
        Path("src/research_graph/infrastructure/graph/r024_networkx_probe.py"),
        "write_text",
        "config.memory_profile_path",
        None,
    ) == ("graph-probe-output", "reviewed graph probe output")


def test_repair_benchmark_outputs_get_precise_category_without_moving_index() -> None:
    assert _classify(
        Path("src/research_graph/infrastructure/repair/chunking_benchmark.py"),
        "write_text",
        "output_dir / 'chunking-benchmark-summary.json'",
        None,
    ) == ("repair-benchmark-output", "reviewed repair benchmark output")
    assert _classify(
        Path("src/research_graph/infrastructure/repair/chunk_baseline_measurement.py"),
        "write_text",
        "index_path",
        None,
    ) == ("caller-owned-index", "caller-provided paired review index output")


def test_validation_batch_outputs_get_precise_category_without_generic_summary_rule() -> None:
    assert _classify(
        Path("src/research_graph/workflows/validation/batch_workflow.py"),
        "write_text",
        "summary_path",
        None,
    ) == ("validation-batch-output", "reviewed validation batch output")
    assert _classify(
        Path("src/research_graph/workflows/validation/batch_workflow.py"),
        "write_text",
        "output_path",
        None,
    ) == ("validation-batch-output", "reviewed validation batch output")
    assert _classify(
        Path("src/research_graph/infrastructure/example_writer.py"),
        "write_text",
        "summary_path",
        None,
    ) == ("caller-owned", "caller-provided or adapter-owned output path")


def test_unreviewed_summary_artifact_cache_and_destination_paths_keep_broad_categories() -> None:
    assert _classify(
        Path("src/research_graph/infrastructure/example_writer.py"),
        "write_text",
        "summary_path",
        None,
    ) == ("caller-owned", "caller-provided or adapter-owned output path")
    assert _classify(
        Path("src/research_graph/infrastructure/example_writer.py"),
        "write_text",
        "output_dir / 'source-asset-summary.json'",
        None,
    ) == ("run-scoped", "caller/output scoped artifact path")
    assert _classify(
        Path("src/research_graph/infrastructure/example_writer.py"),
        "write_text",
        "cache_path",
        None,
    ) == ("caller-owned", "caller-provided or adapter-owned output path")
    assert _classify(
        Path("src/research_graph/infrastructure/example_writer.py"),
        "write_text",
        "destination",
        None,
    ) == ("caller-owned", "caller-provided or adapter-owned output path")
    assert _classify(
        Path("src/research_graph/infrastructure/graph/example_probe.py"),
        "write_text",
        "summary_path",
        None,
    ) == ("caller-owned", "caller-provided or adapter-owned output path")
    assert _classify(
        Path("src/research_graph/infrastructure/repair/example_benchmark.py"),
        "write_text",
        "diagnostics_path",
        None,
    ) == ("append-log", "event or diagnostics log path")


def test_unreviewed_state_index_catalog_paths_remain_shared_state() -> None:
    for target in ("state_path", "index_path", "catalog_path"):
        category, reason = _classify(
            Path("src/research_graph/infrastructure/example_writer.py"),
            "write_text",
            target,
            None,
        )
        assert category == "shared-state"
        assert reason == "stable shared state or index path"


def test_render_delta_markdown_reports_totals_and_sorted_category_deltas() -> None:
    baseline = {
        "summary": {
            "total_records": 3,
            "by_category": {"caller-owned": 2, "run-scoped": 1},
        }
    }
    current = {
        "summary": {
            "total_records": 4,
            "by_category": {"caller-owned": 1, "daily-cli-output": 2, "run-scoped": 1},
        }
    }

    markdown = render_delta_markdown(baseline, current)

    assert "Baseline total records: `3`" in markdown
    assert "Current total records: `4`" in markdown
    assert "Total delta: `+1`" in markdown
    assert "| caller-owned | 2 | 1 | -1 |" in markdown
    assert "| daily-cli-output | 0 | 2 | +2 |" in markdown
    assert "| run-scoped | 1 | 1 | +0 |" in markdown
    assert markdown.index("caller-owned") < markdown.index("daily-cli-output") < markdown.index(
        "run-scoped"
    )


def test_m184_canonical_inventory_ratchets_script_only_without_guardrail_regression() -> None:
    canonical = json.loads(
        Path("data/architecture-assessment/write-path-inventory-canonical.json").read_text()
    )
    categories = canonical["summary"]["by_category"]

    assert categories.get("script-only", 0) <= 4
    assert categories.get("unknown", 0) == 0
    assert categories.get("shared-state", 0) == 0
