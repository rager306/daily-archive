# M080 Module Inventory

Root: `src/arxiv_archive`

## top_level_modules

Count: **68**

- `analytics.py`
- `article_artifact_metrics.py`
- `article_artifact_minimax.py`
- `article_artifact_reducer.py`
- `article_artifact_worker.py`
- `article_artifacts.py`
- `article_assets.py`
- `article_batch_validation.py`
- `article_evidence_bridge.py`
- `article_links_dedup.py`
- `article_loader.py`
- `article_page_index.py`
- `article_retrieval_tables.py`
- `arxiv_client.py`
- `bounded_chunk_repair.py`
- `candidate_locators.py`
- `chunk_baseline_measurement.py`
- `chunk_import_contract.py`
- `chunk_repair_contract.py`
- `chunking_benchmark.py`
- `cli.py`
- `dspy_extraction.py`
- `embedder.py`
- `evaluation.py`
- `evidence.py`
- `extraction_benchmark.py`
- `full_text.py`
- `graph_readiness.py`
- `graph_readiness_export.py`
- `graph_readiness_extraction_gate.py`
- `graph_readiness_manifest.py`
- `graph_readiness_persistence.py`
- `graph_readiness_retrieval_validation.py`
- `graph_readiness_review.py`
- `hybrid_retrieval.py`
- `import_boundary_rehearsal.py`
- `keyword_extractor.py`
- `ladybug_client.py`
- `llm_provider_config.py`
- `md_converter.py`
- `minimax_structured.py`
- `minimax_usage.py`
- `models_registry.py`
- `page_index.py`
- `pdf_downloader.py`
- `reviewer_packet_prototype.py`
- `rlm_graph_traversal.py`
- `rlm_workflow.py`
- `scientific_extraction.py`
- `scoring.py`
- `semantic_scholar.py`
- `source_asset_manifest.py`
- `structure_aware_chunking.py`
- `summarizer.py`
- `telegram_sender.py`
- `thirty_paper_deviation_scan.py`
- `thirty_paper_source_scan.py`
- `universal_kb_contracts.py`
- `universal_kb_queue.py`
- `universal_kb_rehearsal.py`
- `universal_kb_review_assistance.py`
- `universal_kb_sidecar_boundary.py`
- `universal_kb_smoke.py`
- `universal_kb_substrate_rehearsal.py`
- `validation_batch_provenance.py`
- `validation_batch_state.py`
- `validation_batch_workflow.py`
- `validation_logging.py`

## Existing subpackages

- `assets`: 3 Python files
- `chunking`: 4 Python files
- `identity`: 3 Python files
- `indexing`: 3 Python files
- `ingestion`: 4 Python files
- `llm`: 2 Python files
- `parsing`: 4 Python files
- `quality`: 6 Python files
- `staging`: 3 Python files

## Internal import edges

Total edges detected: **147**

### Most imported targets

- `arxiv_archive.full_text`: 7 incoming imports
- `arxiv_archive.universal_kb_contracts`: 6 incoming imports
- `arxiv_archive.graph_readiness`: 6 incoming imports
- `arxiv_archive.parsing.structure`: 5 incoming imports
- `arxiv_archive.article_artifacts`: 4 incoming imports
- `arxiv_archive.validation_batch_state`: 4 incoming imports
- `arxiv_archive.ingestion.loader`: 4 incoming imports
- `arxiv_archive.evidence`: 4 incoming imports
- `arxiv_archive.indexing.page_index`: 4 incoming imports
- `arxiv_archive.models_registry`: 3 incoming imports
- `arxiv_archive.article_artifact_minimax`: 3 incoming imports
- `arxiv_archive.chunk_import_contract`: 3 incoming imports
- `arxiv_archive.page_index`: 3 incoming imports
- `arxiv_archive.scientific_extraction`: 3 incoming imports
- `arxiv_archive.identity.canonicalization`: 3 incoming imports
- `arxiv_archive.indexing.navigation`: 3 incoming imports
- `arxiv_archive.validation_logging`: 3 incoming imports
- `arxiv_archive.parsing.normalization`: 3 incoming imports
- `arxiv_archive.quality.thresholds`: 3 incoming imports
- `arxiv_archive.cli`: 2 incoming imports

### Most connected source modules

- `arxiv_archive.cli`: 11 internal imports
- `arxiv_archive.article_evidence_bridge`: 5 internal imports
- `arxiv_archive.universal_kb_rehearsal`: 5 internal imports
- `arxiv_archive.chunk_baseline_measurement`: 4 internal imports
- `arxiv_archive.graph_readiness_export`: 4 internal imports
- `arxiv_archive.indexing.page_index`: 4 internal imports
- `arxiv_archive.ladybug_client`: 4 internal imports
- `arxiv_archive.quality.maintainability_report`: 4 internal imports
- `arxiv_archive.validation_batch_workflow`: 4 internal imports
- `arxiv_archive.article_artifact_minimax`: 3 internal imports
- `arxiv_archive.chunking.__init__`: 3 internal imports
- `arxiv_archive.chunking.chunker`: 3 internal imports
- `arxiv_archive.evaluation`: 3 internal imports
- `arxiv_archive.graph_readiness_review`: 3 internal imports
- `arxiv_archive.ingestion.__init__`: 3 internal imports
- `arxiv_archive.parsing.__init__`: 3 internal imports
- `arxiv_archive.parsing.parser`: 3 internal imports
- `arxiv_archive.quality.__init__`: 3 internal imports
- `arxiv_archive.rlm_workflow`: 3 internal imports
- `arxiv_archive.article_artifact_worker`: 2 internal imports

## Candidate clusters

### article_artifacts
- `article_artifact_metrics.py`
- `article_artifact_minimax.py`
- `article_artifact_reducer.py`
- `article_artifact_worker.py`
- `article_artifacts.py`
- `article_assets.py`
### graph_readiness
- `graph_readiness.py`
- `graph_readiness_export.py`
- `graph_readiness_extraction_gate.py`
- `graph_readiness_manifest.py`
- `graph_readiness_persistence.py`
- `graph_readiness_retrieval_validation.py`
- `graph_readiness_review.py`
### queue
- `universal_kb_contracts.py`
- `universal_kb_queue.py`
- `universal_kb_rehearsal.py`
- `universal_kb_review_assistance.py`
- `universal_kb_sidecar_boundary.py`
- `universal_kb_smoke.py`
- `universal_kb_substrate_rehearsal.py`
### extraction_benchmark
- `chunking_benchmark.py`
- `extraction_benchmark.py`
### llm
- `dspy_extraction.py`
- `llm_provider_config.py`
- `minimax_structured.py`
- `minimax_usage.py`
- `models_registry.py`
### retrieval
- `article_retrieval_tables.py`
- `embedder.py`
- `hybrid_retrieval.py`
- `keyword_extractor.py`
### catalog_ingestion
- `article_loader.py`
- `arxiv_client.py`
- `full_text.py`
### chunking_repair
- `bounded_chunk_repair.py`
- `chunk_baseline_measurement.py`
- `chunk_import_contract.py`
- `chunk_repair_contract.py`
- `structure_aware_chunking.py`
### cli_and_ops
- `analytics.py`
- `cli.py`
- `ladybug_client.py`
### uncategorized
- `article_batch_validation.py`
- `article_evidence_bridge.py`
- `article_links_dedup.py`
- `article_page_index.py`
- `candidate_locators.py`
- `evaluation.py`
- `evidence.py`
- `import_boundary_rehearsal.py`
- `md_converter.py`
- `page_index.py`
- `pdf_downloader.py`
- `reviewer_packet_prototype.py`
- `rlm_graph_traversal.py`
- `rlm_workflow.py`
- `scientific_extraction.py`
- `scoring.py`
- `semantic_scholar.py`
- `source_asset_manifest.py`
- `summarizer.py`
- `telegram_sender.py`
- `thirty_paper_deviation_scan.py`
- `thirty_paper_source_scan.py`
- `validation_batch_provenance.py`
- `validation_batch_state.py`
- `validation_batch_workflow.py`
- `validation_logging.py`

## Parse errors

- none
