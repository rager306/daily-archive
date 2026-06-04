# M030 Module Function Readiness Report

Rendered for M030 S03 T02 from the M030 pipeline module inventory and bounded URL selection. This is a readiness report only: it does **not** replay acquisition, parse articles, create chunks, promote graph readiness, write LadybugDB, or claim production ingestion.

## Scope

- Milestone: `M030-abwhdm`
- Slice: `S03`
- Task: `T02` Render readiness report
- Source inventory: `doc/architecture/m030_pipeline_module_inventory.json`
- Source selection: `data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json`
- Machine-readable report: `doc/architecture/m030_module_function_readiness.json`

## Status Legend

| State | Meaning |
|---|---|
| ✅ `ready` | Stage has enough evidence to use as an input contract. |
| 🟨 `partial` | Module exists, but selected refs need more evidence before the stage is fully claimable. |
| 🔮 `future-scope` | Module exists, but this milestone must not claim readiness until upstream replay evidence exists. |
| ⛔ `blocked` | Concrete upstream work must happen before this stage can be replayed or claimed. |
| 🚫 `unsafe-to-claim` | Claiming readiness/import would be false or misleading from current evidence. |

## Selection Readiness Summary

- Requested refs: `4`
- Already cataloged: `2` — `arxiv:2507.19457`, `arxiv:2605.26099`
- Missing from article catalog: `2` — `stanford:cs224n:gradient-notes`, `arxiv:2605.29548`
- Safety flags: source acquisition, parser readiness, chunk readiness, KG readiness, graph writes, and production persistence remain `false`.

## Pipeline Stage Overview

| Stage | Module | Inventory | Claim state | Immediate next action |
|---|---|---|---|---|
| URL intake | `m030_requested_ref_intake` | `covered` | ✅ `ready` | Use selection_id m029-pipeline-architecture-audit-v1 as the input contract for catalog/replay work. |
| Article catalog | `metadata_only_catalog_registration` | `covered` | 🟨 `partial` | Register metadata-only catalog rows for stanford:cs224n:gradient-notes and arxiv:2605.29548. |
| Source acquisition | `mixed_source_capture_boundary` | `covered` | ⛔ `blocked` | After catalog registration, replay controlled acquisition for the two missing refs and include already-cataloged refs in replay scope. |
| Loader evidence | `local_ingestion_loader_and_evidence_bridge` | `covered` | 🟨 `partial` | Run loader replay against all four requested identities after acquisition artifacts exist. |
| Parser/conversion | `conversion_quality_and_parser_boundary` | `covered` | ⛔ `blocked` | Classify the Stanford PDF through the local PDF parser path and record conversion diagnostics. |
| Chunking | `pageindex_semantic_chunk_evidence` | `covered` | 🔮 `future-scope` | Run PageIndex/semantic chunk evidence only after parser output is available and classified as usable. |
| Graph-readiness review | `graph_readiness_export_and_independent_review` | `covered` | 🔮 `future-scope` | Create reviewer packets from chunk evidence and require independent graph-readiness review before import eligibility. |
| Graph import boundary | `fail_closed_import_contract_and_rehearsal` | `covered` | 🚫 `unsafe-to-claim` | Use fail-closed import rehearsal only after graph-readiness review passes. |
| Cross-stage replay | `current_pipeline_and_end_to_end_replay` | `covered` | ⛔ `blocked` | Run a continuous replay from catalog registration through graph-readiness review once upstream blockers are cleared. |

## Stage Details

### ✅ URL intake: `m030_requested_ref_intake`

- Inventory status: `covered`
- Claim state: `ready`
- Rationale: The bounded URL selection exists, preserves four requested refs, and keeps downstream unsafe claims false.

**Primary functions/classes**

- `scripts/verify_m030_requested_ref_intake.py:validate_selection`
- `scripts/verify_m030_requested_ref_intake.py:validate_report`
- `scripts/verify_m030_requested_ref_intake.py:validate_catalog_status`
- `scripts/verify_m030_requested_ref_intake.py:validate_m028_status`

**Unsafe-to-claim functions for M030 selected refs**

- None from this stage; current evidence is sufficient for the limited claim shown above.

**Tests/verifiers**

- `scripts/verify_m030_requested_ref_intake.py --validate-only`

**Observability surfaces**

- Stable M030_INTAKE_* diagnostic codes
- Success line reports refs/cataloged/missing counts and fail-closed status

**Concrete next actions**

- Use selection_id m029-pipeline-architecture-audit-v1 as the input contract for catalog/replay work.
- Keep unsafe_claims false until each downstream stage has replay evidence for the selected refs.

### 🟨 Article catalog: `metadata_only_catalog_registration`

- Inventory status: `covered`
- Claim state: `partial`
- Rationale: 2 of 4 requested refs are cataloged; 2 still require metadata-only registration before acquisition replay.

**Primary functions/classes**

- `scripts/register_m027_mixed_source_corpus.py:ArticleSpec`
- `scripts/register_m027_mixed_source_corpus.py:_default_arxiv_strategy`
- `scripts/register_m027_mixed_source_corpus.py:_default_arxiv_variants`
- `scripts/register_m027_mixed_source_corpus.py:_article_record`
- `scripts/register_m027_mixed_source_corpus.py:_selection_payload`
- `scripts/register_m027_mixed_source_corpus.py:register`

**Unsafe-to-claim functions for M030 selected refs**

- `scripts/register_m027_mixed_source_corpus.py:ArticleSpec`
- `scripts/register_m027_mixed_source_corpus.py:_default_arxiv_strategy`
- `scripts/register_m027_mixed_source_corpus.py:_default_arxiv_variants`
- `scripts/register_m027_mixed_source_corpus.py:_article_record`
- `scripts/register_m027_mixed_source_corpus.py:_selection_payload`
- `scripts/register_m027_mixed_source_corpus.py:register`

**Tests/verifiers**

- `tests/test_m027_mixed_source_catalog.py`
- `tests/test_article_catalog_schema.py`
- `scripts/verify_m025_article_catalog.py`

**Observability surfaces**

- Registration diagnostics include selection_id, article_ref, seed_url, fail_closed_safety_flags, and network_fetch_attempted=false

**Concrete next actions**

- Register metadata-only catalog rows for stanford:cs224n:gradient-notes and arxiv:2605.29548.
- Rebuild/validate article_catalog index rows and keep network_fetch_attempted=false during registration.

### ⛔ Source acquisition: `mixed_source_capture_boundary`

- Inventory status: `covered`
- Claim state: `blocked`
- Rationale: The acquisition boundary is implemented, but M030 requested refs cannot be replayed end-to-end until missing catalog records exist.

**Primary functions/classes**

- `scripts/capture_m027_mixed_source_sources.py:FetchResponse`
- `scripts/capture_m027_mixed_source_sources.py:default_fetcher`
- `scripts/capture_m027_mixed_source_sources.py:fixture_response_fetcher`
- `scripts/capture_m027_mixed_source_sources.py:target_path_for_variant`
- `scripts/capture_m027_mixed_source_sources.py:diagnostic_result`
- `scripts/verify_m027_source_acquisition_boundary.py:validate_captured_variant`

**Unsafe-to-claim functions for M030 selected refs**

- `scripts/capture_m027_mixed_source_sources.py:FetchResponse`
- `scripts/capture_m027_mixed_source_sources.py:default_fetcher`
- `scripts/capture_m027_mixed_source_sources.py:fixture_response_fetcher`
- `scripts/capture_m027_mixed_source_sources.py:target_path_for_variant`
- `scripts/capture_m027_mixed_source_sources.py:diagnostic_result`
- `scripts/verify_m027_source_acquisition_boundary.py:validate_captured_variant`

**Tests/verifiers**

- `tests/test_m027_source_acquisition_boundary.py`
- `scripts/verify_m027_source_acquisition_boundary.py`

**Observability surfaces**

- Per-variant diagnostics with diagnostic_code, status, sha256, byte_size, media_type, and safety flags
- Summary counts for captured/blocked/failed

**Concrete next actions**

- After catalog registration, replay controlled acquisition for the two missing refs and include already-cataloged refs in replay scope.
- Persist loader/acquisition events and summary artifacts with source_artifact_captured updated only from real local captures.

### 🟨 Loader evidence: `local_ingestion_loader_and_evidence_bridge`

- Inventory status: `covered`
- Claim state: `partial`
- Rationale: Loader evidence bridge exists, but evidence for the full requested set depends on catalog registration plus acquisition replay.

**Primary functions/classes**

- `src/arxiv_archive/ingestion/loader.py:FullTextSource`
- `src/arxiv_archive/ingestion/loader.py:FullTextIngestionResult`
- `src/arxiv_archive/ingestion/loader.py:ArticleLoadSource`
- `src/arxiv_archive/ingestion/loader.py:ArticleLoadResult`
- `src/arxiv_archive/ingestion/loader.py:ingest_full_text`
- `src/arxiv_archive/ingestion/loader.py:load_article_source`
- `src/arxiv_archive/article_evidence_bridge.py:build_article_evidence_bundle_from_load_events`

**Unsafe-to-claim functions for M030 selected refs**

- `src/arxiv_archive/ingestion/loader.py:FullTextSource`
- `src/arxiv_archive/ingestion/loader.py:FullTextIngestionResult`
- `src/arxiv_archive/ingestion/loader.py:ArticleLoadSource`
- `src/arxiv_archive/ingestion/loader.py:ArticleLoadResult`
- `src/arxiv_archive/ingestion/loader.py:ingest_full_text`
- `src/arxiv_archive/ingestion/loader.py:load_article_source`
- `src/arxiv_archive/article_evidence_bridge.py:build_article_evidence_bundle_from_load_events`

**Tests/verifiers**

- `tests/test_full_text_ingestion.py`
- `tests/test_article_evidence_bridge.py`

**Observability surfaces**

- FullTextQualityReport counters and warnings
- ArticleLoadResult duration_ms, warning_count, outcome, failure_reason, source_id, checksum

**Concrete next actions**

- Run loader replay against all four requested identities after acquisition artifacts exist.
- Verify every successful load has provenance, source path, diagnostics, and fail-closed safety flags.

### ⛔ Parser/conversion: `conversion_quality_and_parser_boundary`

- Inventory status: `covered`
- Claim state: `blocked`
- Rationale: Parser/conversion quality checks exist, but parser readiness is explicitly false for this selection and cannot be claimed before local source replay.

**Primary functions/classes**

- `scripts/convert_m027_source_quality_boundary.py:verify_source_bytes`
- `scripts/convert_m027_source_quality_boundary.py:converted_text_path`
- `scripts/verify_m027_conversion_quality_boundary.py:verify`
- `scripts/verify_m027_conversion_quality_boundary.py:validate_row_semantics`
- `src/arxiv_archive/parsing/parser.py:parse_article`
- `src/arxiv_archive/parsing/parser.py:_fallback_article`

**Unsafe-to-claim functions for M030 selected refs**

- `scripts/convert_m027_source_quality_boundary.py:verify_source_bytes`
- `scripts/convert_m027_source_quality_boundary.py:converted_text_path`
- `scripts/verify_m027_conversion_quality_boundary.py:verify`
- `scripts/verify_m027_conversion_quality_boundary.py:validate_row_semantics`
- `src/arxiv_archive/parsing/parser.py:parse_article`
- `src/arxiv_archive/parsing/parser.py:_fallback_article`

**Tests/verifiers**

- `tests/test_m027_conversion_quality_boundary.py`
- `scripts/verify_m027_conversion_quality_boundary.py`
- `tests/test_page_index.py`

**Observability surfaces**

- diagnostic_code per conversion row
- quality status/counters and structure_counts
- source and converted payload hashes/byte sizes
- Failure Modes, Load Profile, and Negative Tests report sections required by verifier

**Concrete next actions**

- Classify the Stanford PDF through the local PDF parser path and record conversion diagnostics.
- Replay arXiv PDF/HTML conversion quality for arxiv:2605.29548 and the already-cataloged arXiv refs.

### 🔮 Chunking: `pageindex_semantic_chunk_evidence`

- Inventory status: `covered`
- Claim state: `future-scope`
- Rationale: Chunking modules and verifiers exist, but chunk readiness for these refs must wait for parser/conversion evidence.

**Primary functions/classes**

- `src/arxiv_archive/indexing/page_index.py:build_page_index`
- `src/arxiv_archive/indexing/page_index.py:build_page_index_from_parsed`
- `src/arxiv_archive/evidence.py:SemanticChunk`
- `src/arxiv_archive/evidence.py:EvidencePath`
- `src/arxiv_archive/evidence.py:build_semantic_chunks`
- `src/arxiv_archive/evidence.py:build_evidence_paths`
- `src/arxiv_archive/evidence.py:validate_evidence_path`

**Unsafe-to-claim functions for M030 selected refs**

- `src/arxiv_archive/indexing/page_index.py:build_page_index`
- `src/arxiv_archive/indexing/page_index.py:build_page_index_from_parsed`
- `src/arxiv_archive/evidence.py:SemanticChunk`
- `src/arxiv_archive/evidence.py:EvidencePath`
- `src/arxiv_archive/evidence.py:build_semantic_chunks`
- `src/arxiv_archive/evidence.py:build_evidence_paths`
- `src/arxiv_archive/evidence.py:validate_evidence_path`

**Tests/verifiers**

- `tests/test_evidence_paths.py`
- `tests/test_page_index.py`
- `scripts/replay_m027_end_to_end_mixed_replay.py`

**Observability surfaces**

- PageIndex validation_warnings
- SemanticChunk provenance fields
- EvidencePath validation_warnings
- Replay chunk_count and evidence_path_count

**Concrete next actions**

- Run PageIndex/semantic chunk evidence only after parser output is available and classified as usable.
- Check chunk IDs, section anchors, bounds, and repair diagnostics before claiming chunk readiness.

### 🔮 Graph-readiness review: `graph_readiness_export_and_independent_review`

- Inventory status: `covered`
- Claim state: `future-scope`
- Rationale: Graph-readiness review/export tooling exists, but KG readiness is false until chunk evidence receives independent review.

**Primary functions/classes**

- `src/arxiv_archive/graph_readiness_export.py:export_corpus`
- `src/arxiv_archive/graph_readiness_export.py:build_package_from_manifest_document`
- `src/arxiv_archive/graph_readiness_export.py:_graph_ready_chunks`
- `src/arxiv_archive/graph_readiness_export.py:_evidence_path_refs`
- `src/arxiv_archive/graph_readiness_export.py:_report`
- `src/arxiv_archive/graph_readiness_review.py:generate_review_bundles`
- `src/arxiv_archive/graph_readiness_review.py:select_review_papers`
- `src/arxiv_archive/graph_readiness_review.py:render_review_bundle`
- `src/arxiv_archive/graph_readiness_review.py:validate_review_artifacts`

**Unsafe-to-claim functions for M030 selected refs**

- `src/arxiv_archive/graph_readiness_export.py:export_corpus`
- `src/arxiv_archive/graph_readiness_export.py:build_package_from_manifest_document`
- `src/arxiv_archive/graph_readiness_export.py:_graph_ready_chunks`
- `src/arxiv_archive/graph_readiness_export.py:_evidence_path_refs`
- `src/arxiv_archive/graph_readiness_export.py:_report`
- `src/arxiv_archive/graph_readiness_review.py:generate_review_bundles`
- `src/arxiv_archive/graph_readiness_review.py:select_review_papers`
- `src/arxiv_archive/graph_readiness_review.py:render_review_bundle`
- `src/arxiv_archive/graph_readiness_review.py:validate_review_artifacts`

**Tests/verifiers**

- `tests/test_graph_readiness_export.py`
- `tests/test_graph_readiness_review.py`
- `uv run python -m arxiv_archive.graph_readiness_review --review-dir <review-dir> --events <events.jsonl> --validate-only --require-completed-review`

**Observability surfaces**

- Graph readiness route events
- Summary package counts/states
- Independent review request and summary events
- Stable review artifact validation diagnostics

**Concrete next actions**

- Create reviewer packets from chunk evidence and require independent graph-readiness review before import eligibility.
- Record unsafe-to-claim reasons for any ref lacking source, parser, chunk, or reviewer evidence.

### 🚫 Graph import boundary: `fail_closed_import_contract_and_rehearsal`

- Inventory status: `covered`
- Claim state: `unsafe-to-claim`
- Rationale: Import contract/rehearsal code is covered, but graph_write_attempted and production_persistence_attempted are false; no LadybugDB/import claim is safe.

**Primary functions/classes**

- `src/arxiv_archive/chunk_import_contract.py:validate_import_ready_package`
- `src/arxiv_archive/chunk_import_contract.py:validation_to_dict`
- `src/arxiv_archive/chunk_import_contract.py:ContractValidationResult.import_ready`
- `src/arxiv_archive/staging/import_boundary.py:ImportCandidate`
- `src/arxiv_archive/staging/import_boundary.py:ImportBoundaryRehearsal`
- `src/arxiv_archive/staging/import_boundary.py:build_import_boundary_rehearsal_from_benchmark`
- `src/arxiv_archive/staging/import_boundary.py:validate_import_boundary_rehearsal`
- `src/arxiv_archive/staging/import_boundary.py:write_import_boundary_rehearsal_run`

**Unsafe-to-claim functions for M030 selected refs**

- `src/arxiv_archive/chunk_import_contract.py:validate_import_ready_package`
- `src/arxiv_archive/chunk_import_contract.py:validation_to_dict`
- `src/arxiv_archive/chunk_import_contract.py:ContractValidationResult.import_ready`
- `src/arxiv_archive/staging/import_boundary.py:ImportCandidate`
- `src/arxiv_archive/staging/import_boundary.py:ImportBoundaryRehearsal`
- `src/arxiv_archive/staging/import_boundary.py:build_import_boundary_rehearsal_from_benchmark`
- `src/arxiv_archive/staging/import_boundary.py:validate_import_boundary_rehearsal`
- `src/arxiv_archive/staging/import_boundary.py:write_import_boundary_rehearsal_run`

**Tests/verifiers**

- `tests/test_import_ready_contract.py`
- `tests/test_import_boundary_rehearsal.py`

**Observability surfaces**

- refusal_counts by stable reason
- diagnostics with object_id/object_type/route and blocks_import
- import_ready=false, production_import_attempted=false, ladybugdb_written=false metadata

**Concrete next actions**

- Use fail-closed import rehearsal only after graph-readiness review passes.
- Do not write LadybugDB or production graph state from this readiness report.

### ⛔ Cross-stage replay: `current_pipeline_and_end_to_end_replay`

- Inventory status: `covered`
- Claim state: `blocked`
- Rationale: End-to-end replay is blocked by partial catalog coverage and absent source/parser/chunk/graph evidence for the full requested set.

**Primary functions/classes**

- `scripts/replay_m027_current_pipeline_baseline.py:replay_baseline`
- `scripts/replay_m027_current_pipeline_baseline.py:run_current_pipeline`
- `scripts/replay_m027_end_to_end_mixed_replay.py:replay_end_to_end`
- `scripts/replay_m027_end_to_end_mixed_replay.py:run_boundaries`
- `scripts/replay_m027_end_to_end_mixed_replay.py:build_readiness_decision`

**Unsafe-to-claim functions for M030 selected refs**

- `scripts/replay_m027_current_pipeline_baseline.py:replay_baseline`
- `scripts/replay_m027_current_pipeline_baseline.py:run_current_pipeline`
- `scripts/replay_m027_end_to_end_mixed_replay.py:replay_end_to_end`
- `scripts/replay_m027_end_to_end_mixed_replay.py:run_boundaries`
- `scripts/replay_m027_end_to_end_mixed_replay.py:build_readiness_decision`

**Tests/verifiers**

- `tests/test_m027_current_pipeline_baseline.py`
- `tests/test_m027_end_to_end_mixed_replay.py`
- `scripts/verify_m027_current_pipeline_baseline.py`

**Observability surfaces**

- input_artifacts/output_artifacts with hashes
- diagnostic_counts and baseline_comparison_counts
- events replay_started/variant_replayed/replay_completed
- failure_modes/load_profile embedded in summaries and reports

**Concrete next actions**

- Run a continuous replay from catalog registration through graph-readiness review once upstream blockers are cleared.
- Treat any missing stage artifact as a failure to claim full pipeline readiness.

## Required Next Actions by Workstream

1. Catalog registration: add metadata-only catalog records for `stanford:cs224n:gradient-notes` and `arxiv:2605.29548`; preserve `network_fetch_attempted=false` during registration.
2. Acquisition replay: after registration, run controlled source acquisition for all four requested identities and persist events/summaries before changing any readiness flag.
3. Parser/conversion: classify the Stanford PDF through local PDF parsing and replay arXiv PDF/HTML conversion quality for the selected arXiv refs.
4. Chunking: run PageIndex/semantic chunk evidence only from verified parser output; check stable IDs, section anchors, and repair diagnostics.
5. Graph-readiness review: produce reviewer packets and require independent review before import rehearsal; keep graph writes and production persistence false until review passes.

## Fail-Closed Boundary

The only safe claim from this report is that the pipeline modules and verifiers are inventoried and the readiness gaps are explicit. Full source, parser, chunk, KG, LadybugDB, or production-import readiness remains unsafe to claim until the next actions above produce replay evidence for the selected refs.
