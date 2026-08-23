# M133 Pyrefly Missing Import Suppressions

Schema: `daily-archive-m133-pyrefly-suppressions.v1`

## Counts

| Category | Count |
|---|---:|
| `legacy_script_path_shim` | 162 |
| `non_import_or_complex_suppression` | 14 |
| `normal_scripts_import` | 40 |
| `optional_dependency_or_stub_gap` | 14 |
| `other_missing_import` | 28 |
| `repo_import_resolution_gap` | 2 |

## Normal scripts imports

| Path | Line | Target | Import line |
|---|---:|---|---|
| `mutants/src/research_graph/workflows/universal_kb/smoke.py` | 15 | `scripts.audit_m036_real_corpus_smoke` | `from scripts.audit_m036_real_corpus_smoke import audit_smoke, write_json, write_markdown_report` |
| `mutants/src/research_graph/workflows/universal_kb/smoke.py` | 18 | `scripts.run_m036_real_corpus_no_write_smoke` | `from scripts.run_m036_real_corpus_no_write_smoke import run_smoke` |
| `mutants/src/research_graph/workflows/universal_kb/smoke.py` | 21 | `scripts.select_m036_real_corpus_smoke_batch` | `from scripts.select_m036_real_corpus_smoke_batch import select_entries` |
| `mutants/src/research_graph/workflows/validation/batch_workflow.py` | 34 | `scripts.run_quality_gate` | `from scripts.run_quality_gate import run_quality_gate` |
| `mutants/tests/test_chunk_repair_contract.py` | 19 | `scripts.render_chunk_repair_contract` | `from scripts.render_chunk_repair_contract import main as render_contract_main` |
| `mutants/tests/test_locator_evidence_audit.py` | 11 | `scripts.audit_locator_evidence` | `from scripts.audit_locator_evidence import (` |
| `mutants/tests/test_m022_final_gate.py` | 17 | `scripts.verify_m022_final_gate` | `from scripts.verify_m022_final_gate import (` |
| `mutants/tests/test_m022_final_gate.py` | 25 | `scripts.verify_m022_final_gate` | `from scripts.verify_m022_final_gate import (` |
| `mutants/tests/test_m023_artifact_scaffold_gate.py` | 7 | `scripts.verify_m023_artifact_scaffold_gate` | `from scripts.verify_m023_artifact_scaffold_gate import (` |
| `mutants/tests/test_m036_real_corpus_no_write_smoke.py` | 8 | `scripts.run_m036_real_corpus_no_write_smoke` | `from scripts.run_m036_real_corpus_no_write_smoke import run_smoke` |
| `mutants/tests/test_m036_real_corpus_smoke_audit.py` | 8 | `scripts.audit_m036_real_corpus_smoke` | `from scripts.audit_m036_real_corpus_smoke import audit_smoke, write_markdown_report` |
| `mutants/tests/test_m052_s02_e2e.py` | 7 | `scripts.m052_rlm_e2e` | `from scripts.m052_rlm_e2e import SAFETY_KEYS, run_e2e` |
| `mutants/tests/test_replay_m028_smoke_closeout.py` | 6 | `scripts.replay_m028_smoke_closeout` | `from scripts.replay_m028_smoke_closeout import (` |
| `mutants/tests/test_reviewer_packet_prototype.py` | 23 | `scripts.render_reviewer_packet_prototype` | `from scripts.render_reviewer_packet_prototype import main as render_cli_main` |
| `mutants/tests/test_reviewer_packet_prototype.py` | 26 | `scripts.render_reviewer_packet_prototype` | `from scripts.render_reviewer_packet_prototype import render_prototype_files as render_cli_files` |
| `mutants/tests/test_reviewer_packet_prototype.py` | 29 | `scripts.verify_reviewer_packet_prototype` | `from scripts.verify_reviewer_packet_prototype import main as verify_cli_main` |
| `mutants/tests/test_reviewer_packet_prototype.py` | 32 | `scripts.verify_reviewer_packet_prototype` | `from scripts.verify_reviewer_packet_prototype import verify_files as verify_cli_files` |
| `mutants/tests/test_reviewer_packet_prototype.py` | 416 | `scripts.render_reviewer_packet_prototype` | `import scripts.render_reviewer_packet_prototype as renderer` |
| `mutants/tests/test_riskratchet_gate.py` | 18 | `scripts` | `from scripts import run_quality_gate as quality_gate_runner` |
| `mutants/tests/test_verify_m028_smoke_closeout.py` | 8 | `scripts.verify_m028_smoke_closeout` | `import scripts.verify_m028_smoke_closeout as verifier` |
| `scripts/verify_m022_final_gate.py` | 29 | `scripts.verify_reviewer_packet_prototype` | `from scripts.verify_reviewer_packet_prototype import (  # noqa: E402` |
| `scripts/verify_m027_provenance_and_riskratchet_gate.py` | 27 | `scripts` | `from scripts import run_quality_gate  # noqa: E402` |
| `src/research_graph/workflows/universal_kb/smoke.py` | 15 | `scripts.audit_m036_real_corpus_smoke` | `from scripts.audit_m036_real_corpus_smoke import audit_smoke, write_json, write_markdown_report` |
| `src/research_graph/workflows/universal_kb/smoke.py` | 18 | `scripts.run_m036_real_corpus_no_write_smoke` | `from scripts.run_m036_real_corpus_no_write_smoke import run_smoke` |
| `src/research_graph/workflows/universal_kb/smoke.py` | 21 | `scripts.select_m036_real_corpus_smoke_batch` | `from scripts.select_m036_real_corpus_smoke_batch import select_entries` |
| `src/research_graph/workflows/validation/batch_workflow.py` | 34 | `scripts.run_quality_gate` | `from scripts.run_quality_gate import run_quality_gate` |
| `tests/test_chunk_repair_contract.py` | 19 | `scripts.render_chunk_repair_contract` | `from scripts.render_chunk_repair_contract import main as render_contract_main` |
| `tests/test_locator_evidence_audit.py` | 11 | `scripts.audit_locator_evidence` | `from scripts.audit_locator_evidence import (` |
| `tests/test_m022_final_gate.py` | 17 | `scripts.verify_m022_final_gate` | `from scripts.verify_m022_final_gate import (` |
| `tests/test_m022_final_gate.py` | 25 | `scripts.verify_m022_final_gate` | `from scripts.verify_m022_final_gate import (` |
| `tests/test_m023_artifact_scaffold_gate.py` | 7 | `scripts.verify_m023_artifact_scaffold_gate` | `from scripts.verify_m023_artifact_scaffold_gate import (` |
| `tests/test_m052_s02_e2e.py` | 7 | `scripts.m052_rlm_e2e` | `from scripts.m052_rlm_e2e import SAFETY_KEYS, run_e2e` |
| `tests/test_reviewer_packet_prototype.py` | 23 | `scripts.render_reviewer_packet_prototype` | `from scripts.render_reviewer_packet_prototype import main as render_cli_main` |
| `tests/test_reviewer_packet_prototype.py` | 26 | `scripts.render_reviewer_packet_prototype` | `from scripts.render_reviewer_packet_prototype import render_prototype_files as render_cli_files` |
| `tests/test_reviewer_packet_prototype.py` | 29 | `scripts.verify_reviewer_packet_prototype` | `from scripts.verify_reviewer_packet_prototype import main as verify_cli_main` |
| `tests/test_reviewer_packet_prototype.py` | 32 | `scripts.verify_reviewer_packet_prototype` | `from scripts.verify_reviewer_packet_prototype import verify_files as verify_cli_files` |
| `tests/test_reviewer_packet_prototype.py` | 416 | `scripts.render_reviewer_packet_prototype` | `import scripts.render_reviewer_packet_prototype as renderer` |
| `tests/test_riskratchet_gate.py` | 18 | `scripts` | `from scripts import run_quality_gate as quality_gate_runner` |
| `tests/test_test_architecture_guardrail.py` | 5 | `scripts` | `from scripts import verify_test_architecture as guardrail` |
| `tests/test_verify_m028_smoke_closeout.py` | 8 | `scripts.verify_m028_smoke_closeout` | `import scripts.verify_m028_smoke_closeout as verifier` |

## Legacy script path shims sample

| Path | Line | Target | Import line |
|---|---:|---|---|
| `mutants/tests/test_acquire_linked_target_pdfs.py` | 25 | `acquire_linked_target_pdfs` | `import acquire_linked_target_pdfs  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m027_conversion_quality_boundary.py` | 14 | `convert_m027_source_quality_boundary` | `from convert_m027_source_quality_boundary import (  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m027_conversion_quality_boundary.py` | 22 | `verify_m027_conversion_quality_boundary` | `from verify_m027_conversion_quality_boundary import (  # ty: ignore[unresolved-import]` |
| `mutants/tests/test_m027_source_acquisition_boundary.py` | 12 | `capture_m027_mixed_source_sources` | `from capture_m027_mixed_source_sources import (  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m027_source_acquisition_boundary.py` | 24 | `verify_m027_source_acquisition_boundary` | `from verify_m027_source_acquisition_boundary import (  # ty: ignore[unresolved-import]` |
| `mutants/tests/test_m029_conversion_quality_boundary.py` | 14 | `verify_m029_unified_conversion_quality_boundary` | `from verify_m029_unified_conversion_quality_boundary import (  # ty: ignore[unresolved-import]` |
| `mutants/tests/test_m029_conversion_quality_boundary.py` | 19 | `verify_m029_unified_conversion_quality_boundary` | `from verify_m029_unified_conversion_quality_boundary import (  # ty: ignore[unresolved-import]` |
| `mutants/tests/test_m029_loader_runtime_smoke.py` | 12 | `run_m029_unified_loader_runtime_smoke` | `from run_m029_unified_loader_runtime_smoke import (  # ty: ignore[unresolved-import]` |
| `mutants/tests/test_m029_loader_runtime_smoke.py` | 17 | `verify_m029_unified_loader_runtime_smoke` | `from verify_m029_unified_loader_runtime_smoke import (  # ty: ignore[unresolved-import]` |
| `mutants/tests/test_m029_source_acquisition.py` | 13 | `capture_m029_unified_sources` | `from capture_m029_unified_sources import (  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m029_source_acquisition.py` | 19 | `verify_m029_unified_source_acquisition` | `from verify_m029_unified_source_acquisition import (  # ty: ignore[unresolved-import]` |
| `mutants/tests/test_m029_unified_readiness.py` | 12 | `synthesize_m029_unified_readiness` | `from synthesize_m029_unified_readiness import (  # ty: ignore[unresolved-import]` |
| `mutants/tests/test_m029_unified_readiness.py` | 17 | `verify_m029_unified_readiness` | `from verify_m029_unified_readiness import (  # ty: ignore[unresolved-import]` |
| `mutants/tests/test_m029_unified_replay.py` | 12 | `run_m029_unified_replay` | `from run_m029_unified_replay import main as run_main  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m029_unified_replay.py` | 14 | `verify_m029_unified_replay` | `from verify_m029_unified_replay import (  # ty: ignore[unresolved-import]  # pyrefly: ignore [missing-import]` |
| `mutants/tests/test_m031_catalog_backed_acquisition_loader.py` | 13 | `build_m031_catalog_backed_replay_selection` | `from build_m031_catalog_backed_replay_selection import (  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m031_catalog_backed_acquisition_loader.py` | 18 | `replay_m031_catalog_backed_acquisition` | `from replay_m031_catalog_backed_acquisition import (  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m031_catalog_backed_acquisition_loader.py` | 22 | `replay_m031_catalog_backed_acquisition` | `from replay_m031_catalog_backed_acquisition import (  # ty: ignore[unresolved-import]` |
| `mutants/tests/test_m031_catalog_backed_acquisition_loader.py` | 26 | `replay_m031_catalog_backed_acquisition` | `from replay_m031_catalog_backed_acquisition import (  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m031_catalog_backed_acquisition_loader.py` | 30 | `replay_m031_catalog_backed_loader_evidence` | `from replay_m031_catalog_backed_loader_evidence import (  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m031_catalog_backed_acquisition_loader.py` | 36 | `replay_m031_catalog_backed_loader_evidence` | `from replay_m031_catalog_backed_loader_evidence import (  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m031_catalog_backed_acquisition_loader.py` | 41 | `replay_m031_catalog_backed_loader_evidence` | `from replay_m031_catalog_backed_loader_evidence import (  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m031_catalog_backed_acquisition_loader.py` | 46 | `verify_m031_catalog_backed_replay` | `from verify_m031_catalog_backed_replay import (  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m031_chunk_evidence_replay.py` | 12 | `replay_m031_chunk_evidence` | `from replay_m031_chunk_evidence import (  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m031_chunk_evidence_replay.py` | 19 | `verify_m031_chunk_evidence_replay` | `from verify_m031_chunk_evidence_replay import (  # ty: ignore[unresolved-import]` |
| `mutants/tests/test_m031_parser_conversion_replay.py` | 14 | `replay_m031_parser_conversion` | `from replay_m031_parser_conversion import (  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m031_parser_conversion_replay.py` | 20 | `verify_m031_parser_conversion_replay` | `from verify_m031_parser_conversion_replay import (  # ty: ignore[unresolved-import]` |
| `mutants/tests/test_m031_parser_conversion_replay.py` | 25 | `verify_m031_parser_conversion_replay` | `from verify_m031_parser_conversion_replay import (  # ty: ignore[unresolved-import]` |
| `mutants/tests/test_m031_parser_conversion_replay.py` | 438 | `replay_m031_parser_conversion` | `import replay_m031_parser_conversion as replay  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m033_opendataloader_adaptix_adapter.py` | 11 | `probe_m033_opendataloader_adaptix_adapter` | `from probe_m033_opendataloader_adaptix_adapter import (  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m033_opendataloader_adaptix_adapter.py` | 17 | `verify_m033_opendataloader_adaptix_adapter` | `from verify_m033_opendataloader_adaptix_adapter import verify  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m053_audit_s02.py` | 14 | `audit_m053_grobid_pilot` | `import audit_m053_grobid_pilot as audit  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m053_audit_s02.py` | 16 | `update_m043_target_subset_post_m053` | `import update_m043_target_subset_post_m053 as update_m043  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m053_grobid_pilot.py` | 22 | `probe_m053_grobid_pilot` | `import probe_m053_grobid_pilot as probe  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m055_benchmark_s01.py` | 18 | `benchmark_m055_availability_probe` | `import benchmark_m055_availability_probe as availability  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m055_benchmark_s01.py` | 20 | `benchmark_m055_corpus_manifest` | `import benchmark_m055_corpus_manifest as corpus  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m055_benchmark_s01.py` | 21 | `benchmark_m055_vendor_check` | `import benchmark_m055_vendor_check as vendor_check  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m055_benchmark_s02.py` | 16 | `benchmark_m055_grobid_only` | `import benchmark_m055_grobid_only as grobid_only  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m055_benchmark_s03.py` | 17 | `benchmark_m055_opendataloader_only` | `import benchmark_m055_opendataloader_only as opendl_only  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m055_benchmark_s04.py` | 14 | `benchmark_m055_hybrid_routing` | `import benchmark_m055_hybrid_routing as hybrid  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m055_benchmark_s05.py` | 11 | `render_m055_report` | `import render_m055_report as report  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m055deep_corpus_20.py` | 13 | `build_m055deep_corpus_manifest_20` | `import build_m055deep_corpus_manifest_20 as corpus20  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m055deep_grobid_fulltext.py` | 15 | `benchmark_m055deep_grobid_fulltext` | `import benchmark_m055deep_grobid_fulltext as fulltext  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m055deep_grobid_fulltext.py` | 17 | `compare_m055_header_vs_fulltext` | `import compare_m055_header_vs_fulltext as compare_delta  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m055deep_hybrid_routing_20.py` | 13 | `benchmark_m055deep_hybrid_routing_20` | `import benchmark_m055deep_hybrid_routing_20 as hybrid20  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m055deep_opendataloader_correctness.py` | 12 | `benchmark_m055deep_opendataloader_correctness` | `import benchmark_m055deep_opendataloader_correctness as correctness  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m055deep_report_s06.py` | 11 | `render_m055deep_report` | `import render_m055deep_report as report  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m057_s01.py` | 12 | `m057_compare_marker_opendataloader` | `import m057_compare_marker_opendataloader as compare  # noqa: E402  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m057_s01.py` | 14 | `m057_fd_validate` | `import m057_fd_validate as fd_validate  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m057_s01.py` | 15 | `m057_marker_extract` | `import m057_marker_extract as marker_extract  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]` |

## Optional dependency or stub gaps sample

| Path | Line | Target | Import line |
|---|---:|---|---|
| `mutants/tests/test_m027_conversion_quality_boundary.py` | 8 | `fitz` | `import fitz  # ty:ignore[unresolved-import]` |
| `mutants/tests/test_m029_conversion_quality_boundary.py` | 8 | `fitz` | `import fitz  # ty:ignore[unresolved-import]` |
| `scripts/m057_marker_extract_5.py` | 15 | `marker.converters.pdf` | `from marker.converters.pdf import PdfConverter  # ty:ignore[unresolved-import]` |
| `scripts/m057_marker_extract_5.py` | 18 | `marker.models` | `from marker.models import create_model_dict  # ty:ignore[unresolved-import]` |
| `scripts/m057_marker_extract_5.py` | 21 | `marker.output` | `from marker.output import text_from_rendered  # ty:ignore[unresolved-import]` |
| `scripts/m058_marker_extract_5.py` | 19 | `marker.config.parser` | `from marker.config.parser import ConfigParser  # ty:ignore[unresolved-import]` |
| `scripts/m058_marker_extract_5.py` | 22 | `marker.converters.pdf` | `from marker.converters.pdf import PdfConverter  # ty:ignore[unresolved-import]` |
| `scripts/m058_marker_extract_5.py` | 25 | `marker.models` | `from marker.models import create_model_dict  # ty:ignore[unresolved-import]` |
| `scripts/m058_marker_extract_5.py` | 28 | `marker.output` | `from marker.output import text_from_rendered  # ty:ignore[unresolved-import]` |
| `scripts/m060b_graph_visualize.py` | 95 | `matplotlib` | `import matplotlib  # ty:ignore[unresolved-import]` |
| `scripts/m060b_graph_visualize.py` | 99 | `matplotlib.pyplot` | `import matplotlib.pyplot as plt  # ty:ignore[unresolved-import]` |
| `scripts/m060b_graph_visualize.py` | 102 | `matplotlib.lines` | `from matplotlib.lines import Line2D  # ty:ignore[unresolved-import]` |
| `tests/test_m027_conversion_quality_boundary.py` | 8 | `fitz` | `import fitz  # ty:ignore[unresolved-import]` |
| `tests/test_m029_conversion_quality_boundary.py` | 8 | `fitz` | `import fitz  # ty:ignore[unresolved-import]` |
