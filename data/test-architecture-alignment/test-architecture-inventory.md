# Test Architecture Inventory

Schema: `daily-archive-test-architecture-inventory.v1`

## Summary

- Total test files: `269`

### Buckets

| Bucket | Count |
|---|---:|
| `domain` | 0 |
| `application` | 10 |
| `infrastructure` | 87 |
| `script-wrapper` | 15 |
| `acceptance` | 6 |
| `legacy-mixed` | 70 |
| `unknown` | 81 |

### Import and execution signals

| Signal | Count |
|---|---:|
| `acceptance_name` | 6 |
| `dynamic_script_import` | 56 |
| `imports_application` | 17 |
| `imports_cli` | 5 |
| `imports_domain` | 14 |
| `imports_infrastructure` | 95 |
| `imports_scripts_normal` | 4 |
| `imports_workflows` | 24 |
| `subprocess_script_invocation` | 16 |

## Representative files by bucket

### domain

- none

### application

- `tests/test_catalog_ingest_use_case.py` — imports application layer without infrastructure
- `tests/test_corpus_coverage_use_case.py` — imports application layer without infrastructure
- `tests/test_extraction_benchmark.py` — imports application layer without infrastructure
- `tests/test_graph_probe_use_case.py` — imports application layer without infrastructure
- `tests/test_m073_parser_evidence_benchmark.py` — imports application layer without infrastructure
- `tests/test_m122_property_mutation_guards.py` — imports application layer without infrastructure
- `tests/test_onion_layering.py` — imports application layer without infrastructure
- `tests/test_parser_replay_use_case.py` — imports application layer without infrastructure
- `tests/test_pipeline_framework.py` — imports application layer without infrastructure
- `tests/test_pipeline_script_inventory.py` — imports application layer without infrastructure

### infrastructure

- `tests/test_analysis.py` — imports infrastructure adapter layer
- `tests/test_analytics.py` — imports infrastructure adapter layer
- `tests/test_article_artifact_metrics.py` — imports infrastructure adapter layer
- `tests/test_article_artifact_minimax.py` — imports infrastructure adapter layer
- `tests/test_article_artifacts.py` — imports infrastructure adapter layer
- `tests/test_article_artifacts_cli.py` — imports infrastructure adapter layer
- `tests/test_article_batch_validation.py` — imports infrastructure adapter layer
- `tests/test_article_evidence_bridge.py` — imports infrastructure adapter layer
- `tests/test_article_loader.py` — imports infrastructure adapter layer
- `tests/test_arxiv_client.py` — imports infrastructure adapter layer

### script-wrapper

- `tests/test_ingest_cli.py` — script wrapper or subprocess invocation
- `tests/test_locator_evidence_audit.py` — script wrapper or subprocess invocation
- `tests/test_m023_artifact_scaffold_gate.py` — script wrapper or subprocess invocation
- `tests/test_m027_mixed_source_catalog.py` — script wrapper or subprocess invocation
- `tests/test_m028_smoke_replay_closeout.py` — script wrapper or subprocess invocation
- `tests/test_m031_import_boundary_rehearsal.py` — script wrapper or subprocess invocation
- `tests/test_m031_process_continuity_audit.py` — script wrapper or subprocess invocation
- `tests/test_m031_s05_closeout.py` — script wrapper or subprocess invocation
- `tests/test_m053_grobid_pilot.py` — script wrapper or subprocess invocation
- `tests/test_m059_s01.py` — script wrapper or subprocess invocation

### acceptance

- `tests/test_m022_final_gate.py` — acceptance marker in file name or content
- `tests/test_m063_s02.py` — acceptance marker in file name or content
- `tests/test_modular_properties.py` — acceptance marker in file name or content
- `tests/test_pipeline_architecture_acceptance.py` — acceptance marker in file name or content
- `tests/test_reviewer_packet_prototype.py` — acceptance marker in file name or content
- `tests/test_test_architecture_guardrail.py` — acceptance marker in file name or content

### legacy-mixed

- `tests/test_article_baseline_recovery_replay.py` — dynamic script import via spec_from_file_location
- `tests/test_article_preprocessing_replay_contract.py` — dynamic script import via spec_from_file_location
- `tests/test_bounded_chunk_repair.py` — dynamic script import via spec_from_file_location
- `tests/test_codebase_memory_governance.py` — dynamic script import via spec_from_file_location
- `tests/test_dspy_extraction_boundary.py` — dynamic script import via spec_from_file_location
- `tests/test_import_boundary_rehearsal.py` — imports workflow, CLI, or legacy pipeline surface
- `tests/test_m024_validation_evidence_closure.py` — dynamic script import via spec_from_file_location
- `tests/test_m025_boundary_replay_completion.py` — dynamic script import via spec_from_file_location
- `tests/test_m025_evidence_replay.py` — dynamic script import via spec_from_file_location
- `tests/test_m025_requirement_scope_reconciliation.py` — dynamic script import via spec_from_file_location

### unknown

- `tests/test_acquire_linked_target_pdfs.py` — no recognized project-layer import signal
- `tests/test_article_assets.py` — no recognized project-layer import signal
- `tests/test_article_catalog_schema.py` — no recognized project-layer import signal
- `tests/test_article_evidence_boundaries.py` — no recognized project-layer import signal
- `tests/test_article_links_dedup.py` — no recognized project-layer import signal
- `tests/test_article_page_index.py` — no recognized project-layer import signal
- `tests/test_article_retrieval_tables.py` — no recognized project-layer import signal
- `tests/test_cli_contract.py` — no recognized project-layer import signal
- `tests/test_m025_article_catalog_verifier.py` — no recognized project-layer import signal
- `tests/test_m027_conversion_quality_boundary.py` — no recognized project-layer import signal

## Pilot candidates

| Path | Current bucket | Suggested layer |
|---|---|---|
| `tests/test_catalog_ingest_filesystem_adapter.py` | `infrastructure` | `infrastructure` |
| `tests/test_catalog_ingest_m056.py` | `infrastructure` | `infrastructure` |
| `tests/test_catalog_ingest_use_case.py` | `application` | `application` |
| `tests/test_corpus_coverage_report_writer.py` | `infrastructure` | `infrastructure` |
| `tests/test_corpus_coverage_use_case.py` | `application` | `application` |
| `tests/test_graph_probe_use_case.py` | `application` | `application` |
| `tests/test_m122_property_mutation_guards.py` | `application` | `application` |
| `tests/test_networkx_graph_probe_adapter.py` | `infrastructure` | `infrastructure` |
| `tests/test_parser_replay_adapters.py` | `infrastructure` | `infrastructure` |
| `tests/test_parser_replay_use_case.py` | `application` | `application` |
| `tests/test_pipeline_architecture_acceptance.py` | `acceptance` | `acceptance` |
| `tests/test_pipeline_script_inventory.py` | `application` | `application` |
| `tests/test_riskratchet_gate.py` | `script-wrapper` | `script-wrapper` |
