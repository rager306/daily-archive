# M030 Requirement Module Matrix

Rendered for M030 S04 T02 from the M030 module-function readiness report. This is a human-readable planning and validation artifact only; it does **not** replay acquisition, parse/chunk articles, write LadybugDB, or claim production ingestion.

## Scope

- Milestone: `M030-abwhdm`
- Slice: `S04` Requirement-module validation
- Task: `T02` Render requirement module report
- Source readiness report: `doc/architecture/m030_module_function_readiness.json`
- Machine-readable matrix: `doc/architecture/m030_requirement_module_matrix.json`
- Behavior changed: `false`
- Runtime replay performed: `false`

## Coverage Categories

| Category | Meaning | Count |
|---|---|---:|
| ✅ `covered` | Enough evidence exists for the limited planning claim in this report. | 1 |
| 🟨 `partial` | The module exists, but selected M030 refs need more evidence before full requirement coverage can be claimed. | 2 |
| 🔮 `future-scope` | The module exists, but this milestone must wait for upstream replay/review evidence before claiming it. | 2 |
| ⛔ `blocked` | A concrete upstream dependency prevents replay or coverage validation now. | 3 |
| 🚫 `unsafe-to-claim` | Current evidence explicitly says not to claim readiness/import/production behavior. | 1 |

## Matrix Overview

| Requirement/stage | Coverage | Module | Current claim | Next movement |
|---|---|---|---|---|
| URL intake | ✅ `covered` | `m030_requested_ref_intake` | `ready` | Use selection_id m029-pipeline-architecture-audit-v1 as the input contract for catalog/replay work. |
| Article catalog | 🟨 `partial` | `metadata_only_catalog_registration` | `partial` | Register metadata-only catalog rows for stanford:cs224n:gradient-notes and arxiv:2605.29548. |
| Source acquisition | ⛔ `blocked` | `mixed_source_capture_boundary` | `blocked` | After catalog registration, replay controlled acquisition for the two missing refs and include already-cataloged refs in replay scope. |
| Loader evidence | 🟨 `partial` | `local_ingestion_loader_and_evidence_bridge` | `partial` | Run loader replay against all four requested identities after acquisition artifacts exist. |
| Parser/conversion | ⛔ `blocked` | `conversion_quality_and_parser_boundary` | `blocked` | Classify the Stanford PDF through the local PDF parser path and record conversion diagnostics. |
| Chunking | 🔮 `future-scope` | `pageindex_semantic_chunk_evidence` | `future-scope` | Run PageIndex/semantic chunk evidence only after parser output is available and classified as usable. |
| Graph-readiness review | 🔮 `future-scope` | `graph_readiness_export_and_independent_review` | `future-scope` | Create reviewer packets from chunk evidence and require independent graph-readiness review before import eligibility. |
| Graph import boundary | 🚫 `unsafe-to-claim` | `fail_closed_import_contract_and_rehearsal` | `unsafe-to-claim` | Use fail-closed import rehearsal only after graph-readiness review passes. |
| Cross-stage replay | ⛔ `blocked` | `current_pipeline_and_end_to_end_replay` | `blocked` | Run a continuous replay from catalog registration through graph-readiness review once upstream blockers are cleared. |

## ✅ Covered

### URL intake: `m030_requested_ref_intake`

- Stage: `url_intake`
- Inventory status: `covered`
- Current claim state: `ready`
- Rationale: The bounded URL selection exists, preserves four requested refs, and keeps downstream unsafe claims false.

**Primary module/functions**

- `scripts/verify_m030_requested_ref_intake.py:validate_selection`
- `scripts/verify_m030_requested_ref_intake.py:validate_report`
- `scripts/verify_m030_requested_ref_intake.py:validate_catalog_status`
- `scripts/verify_m030_requested_ref_intake.py:validate_m028_status`

**Tests/verifiers**

- `scripts/verify_m030_requested_ref_intake.py --validate-only`

**Observability surfaces**

- Stable M030_INTAKE_* diagnostic codes
- Success line reports refs/cataloged/missing counts and fail-closed status

**Next actions**

- Use selection_id m029-pipeline-architecture-audit-v1 as the input contract for catalog/replay work.
- Keep unsafe_claims false until each downstream stage has replay evidence for the selected refs.

## 🟨 Partial

### Article catalog: `metadata_only_catalog_registration`

- Stage: `article_catalog`
- Inventory status: `covered`
- Current claim state: `partial`
- Rationale: 2 of 4 requested refs are cataloged; 2 still require metadata-only registration before acquisition replay.

**Primary module/functions**

- `scripts/register_m027_mixed_source_corpus.py:ArticleSpec`
- `scripts/register_m027_mixed_source_corpus.py:_default_arxiv_strategy`
- `scripts/register_m027_mixed_source_corpus.py:_default_arxiv_variants`
- `scripts/register_m027_mixed_source_corpus.py:_article_record`
- `scripts/register_m027_mixed_source_corpus.py:_selection_payload`
- `scripts/register_m027_mixed_source_corpus.py:register`

**Module/function that must move next**

- `metadata_only_catalog_registration` via `scripts/register_m027_mixed_source_corpus.py:ArticleSpec` — Register metadata-only catalog rows for stanford:cs224n:gradient-notes and arxiv:2605.29548.
- `metadata_only_catalog_registration` via `scripts/register_m027_mixed_source_corpus.py:_default_arxiv_strategy` — Rebuild/validate article_catalog index rows and keep network_fetch_attempted=false during registration.

**Tests/verifiers**

- `tests/test_m027_mixed_source_catalog.py`
- `tests/test_article_catalog_schema.py`
- `scripts/verify_m025_article_catalog.py`

**Observability surfaces**

- Registration diagnostics include selection_id, article_ref, seed_url, fail_closed_safety_flags, and network_fetch_attempted=false

**Next actions**

- Register metadata-only catalog rows for stanford:cs224n:gradient-notes and arxiv:2605.29548.
- Rebuild/validate article_catalog index rows and keep network_fetch_attempted=false during registration.

### Loader evidence: `local_ingestion_loader_and_evidence_bridge`

- Stage: `loader_evidence`
- Inventory status: `covered`
- Current claim state: `partial`
- Rationale: Loader evidence bridge exists, but evidence for the full requested set depends on catalog registration plus acquisition replay.

**Primary module/functions**

- `src/arxiv_archive/ingestion/loader.py:FullTextSource`
- `src/arxiv_archive/ingestion/loader.py:FullTextIngestionResult`
- `src/arxiv_archive/ingestion/loader.py:ArticleLoadSource`
- `src/arxiv_archive/ingestion/loader.py:ArticleLoadResult`
- `src/arxiv_archive/ingestion/loader.py:ingest_full_text`
- `src/arxiv_archive/ingestion/loader.py:load_article_source`
- `src/arxiv_archive/article_evidence_bridge.py:build_article_evidence_bundle_from_load_events`

**Module/function that must move next**

- `local_ingestion_loader_and_evidence_bridge` via `src/arxiv_archive/ingestion/loader.py:FullTextSource` — Run loader replay against all four requested identities after acquisition artifacts exist.
- `local_ingestion_loader_and_evidence_bridge` via `src/arxiv_archive/ingestion/loader.py:FullTextIngestionResult` — Verify every successful load has provenance, source path, diagnostics, and fail-closed safety flags.

**Tests/verifiers**

- `tests/test_full_text_ingestion.py`
- `tests/test_article_evidence_bridge.py`

**Observability surfaces**

- FullTextQualityReport counters and warnings
- ArticleLoadResult duration_ms, warning_count, outcome, failure_reason, source_id, checksum

**Next actions**

- Run loader replay against all four requested identities after acquisition artifacts exist.
- Verify every successful load has provenance, source path, diagnostics, and fail-closed safety flags.

## 🔮 Future scope

### Chunking: `pageindex_semantic_chunk_evidence`

- Stage: `chunking`
- Inventory status: `covered`
- Current claim state: `future-scope`
- Rationale: Chunking modules and verifiers exist, but chunk readiness for these refs must wait for parser/conversion evidence.

**Primary module/functions**

- `src/arxiv_archive/indexing/page_index.py:build_page_index`
- `src/arxiv_archive/indexing/page_index.py:build_page_index_from_parsed`
- `src/arxiv_archive/evidence.py:SemanticChunk`
- `src/arxiv_archive/evidence.py:EvidencePath`
- `src/arxiv_archive/evidence.py:build_semantic_chunks`
- `src/arxiv_archive/evidence.py:build_evidence_paths`
- `src/arxiv_archive/evidence.py:validate_evidence_path`

**Module/function that must move next**

- `pageindex_semantic_chunk_evidence` via `src/arxiv_archive/indexing/page_index.py:build_page_index` — Run PageIndex/semantic chunk evidence only after parser output is available and classified as usable.
- `pageindex_semantic_chunk_evidence` via `src/arxiv_archive/indexing/page_index.py:build_page_index_from_parsed` — Check chunk IDs, section anchors, bounds, and repair diagnostics before claiming chunk readiness.

**Tests/verifiers**

- `tests/test_evidence_paths.py`
- `tests/test_page_index.py`
- `scripts/replay_m027_end_to_end_mixed_replay.py`

**Observability surfaces**

- PageIndex validation_warnings
- SemanticChunk provenance fields
- EvidencePath validation_warnings
- Replay chunk_count and evidence_path_count

**Next actions**

- Run PageIndex/semantic chunk evidence only after parser output is available and classified as usable.
- Check chunk IDs, section anchors, bounds, and repair diagnostics before claiming chunk readiness.

### Graph-readiness review: `graph_readiness_export_and_independent_review`

- Stage: `graph_readiness_review`
- Inventory status: `covered`
- Current claim state: `future-scope`
- Rationale: Graph-readiness review/export tooling exists, but KG readiness is false until chunk evidence receives independent review.

**Primary module/functions**

- `src/arxiv_archive/graph_readiness_export.py:export_corpus`
- `src/arxiv_archive/graph_readiness_export.py:build_package_from_manifest_document`
- `src/arxiv_archive/graph_readiness_export.py:_graph_ready_chunks`
- `src/arxiv_archive/graph_readiness_export.py:_evidence_path_refs`
- `src/arxiv_archive/graph_readiness_export.py:_report`
- `src/arxiv_archive/graph_readiness_review.py:generate_review_bundles`
- `src/arxiv_archive/graph_readiness_review.py:select_review_papers`
- `src/arxiv_archive/graph_readiness_review.py:render_review_bundle`
- `src/arxiv_archive/graph_readiness_review.py:validate_review_artifacts`

**Module/function that must move next**

- `graph_readiness_export_and_independent_review` via `src/arxiv_archive/graph_readiness_export.py:export_corpus` — Create reviewer packets from chunk evidence and require independent graph-readiness review before import eligibility.
- `graph_readiness_export_and_independent_review` via `src/arxiv_archive/graph_readiness_export.py:build_package_from_manifest_document` — Record unsafe-to-claim reasons for any ref lacking source, parser, chunk, or reviewer evidence.

**Tests/verifiers**

- `tests/test_graph_readiness_export.py`
- `tests/test_graph_readiness_review.py`
- `uv run python -m arxiv_archive.graph_readiness_review --review-dir <review-dir> --events <events.jsonl> --validate-only --require-completed-review`

**Observability surfaces**

- Graph readiness route events
- Summary package counts/states
- Independent review request and summary events
- Stable review artifact validation diagnostics

**Next actions**

- Create reviewer packets from chunk evidence and require independent graph-readiness review before import eligibility.
- Record unsafe-to-claim reasons for any ref lacking source, parser, chunk, or reviewer evidence.

## ⛔ Blocked

### Source acquisition: `mixed_source_capture_boundary`

- Stage: `source_acquisition`
- Inventory status: `covered`
- Current claim state: `blocked`
- Rationale: The acquisition boundary is implemented, but M030 requested refs cannot be replayed end-to-end until missing catalog records exist.

**Primary module/functions**

- `scripts/capture_m027_mixed_source_sources.py:FetchResponse`
- `scripts/capture_m027_mixed_source_sources.py:default_fetcher`
- `scripts/capture_m027_mixed_source_sources.py:fixture_response_fetcher`
- `scripts/capture_m027_mixed_source_sources.py:target_path_for_variant`
- `scripts/capture_m027_mixed_source_sources.py:diagnostic_result`
- `scripts/verify_m027_source_acquisition_boundary.py:validate_captured_variant`

**Unsafe-to-claim boundary**

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

**Next actions**

- After catalog registration, replay controlled acquisition for the two missing refs and include already-cataloged refs in replay scope.
- Persist loader/acquisition events and summary artifacts with source_artifact_captured updated only from real local captures.

### Parser/conversion: `conversion_quality_and_parser_boundary`

- Stage: `parser_conversion`
- Inventory status: `covered`
- Current claim state: `blocked`
- Rationale: Parser/conversion quality checks exist, but parser readiness is explicitly false for this selection and cannot be claimed before local source replay.

**Primary module/functions**

- `scripts/convert_m027_source_quality_boundary.py:verify_source_bytes`
- `scripts/convert_m027_source_quality_boundary.py:converted_text_path`
- `scripts/verify_m027_conversion_quality_boundary.py:verify`
- `scripts/verify_m027_conversion_quality_boundary.py:validate_row_semantics`
- `src/arxiv_archive/parsing/parser.py:parse_article`
- `src/arxiv_archive/parsing/parser.py:_fallback_article`

**Unsafe-to-claim boundary**

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

**Next actions**

- Classify the Stanford PDF through the local PDF parser path and record conversion diagnostics.
- Replay arXiv PDF/HTML conversion quality for arxiv:2605.29548 and the already-cataloged arXiv refs.

### Cross-stage replay: `current_pipeline_and_end_to_end_replay`

- Stage: `cross_stage_replay`
- Inventory status: `covered`
- Current claim state: `blocked`
- Rationale: End-to-end replay is blocked by partial catalog coverage and absent source/parser/chunk/graph evidence for the full requested set.

**Primary module/functions**

- `scripts/replay_m027_current_pipeline_baseline.py:replay_baseline`
- `scripts/replay_m027_current_pipeline_baseline.py:run_current_pipeline`
- `scripts/replay_m027_end_to_end_mixed_replay.py:replay_end_to_end`
- `scripts/replay_m027_end_to_end_mixed_replay.py:run_boundaries`
- `scripts/replay_m027_end_to_end_mixed_replay.py:build_readiness_decision`

**Unsafe-to-claim boundary**

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

**Next actions**

- Run a continuous replay from catalog registration through graph-readiness review once upstream blockers are cleared.
- Treat any missing stage artifact as a failure to claim full pipeline readiness.

## 🚫 Unsafe to claim

### Graph import boundary: `fail_closed_import_contract_and_rehearsal`

- Stage: `graph_import_boundary`
- Inventory status: `covered`
- Current claim state: `unsafe-to-claim`
- Rationale: Import contract/rehearsal code is covered, but graph_write_attempted and production_persistence_attempted are false; no LadybugDB/import claim is safe.

**Primary module/functions**

- `src/arxiv_archive/chunk_import_contract.py:validate_import_ready_package`
- `src/arxiv_archive/chunk_import_contract.py:validation_to_dict`
- `src/arxiv_archive/chunk_import_contract.py:ContractValidationResult.import_ready`
- `src/arxiv_archive/staging/import_boundary.py:ImportCandidate`
- `src/arxiv_archive/staging/import_boundary.py:ImportBoundaryRehearsal`
- `src/arxiv_archive/staging/import_boundary.py:build_import_boundary_rehearsal_from_benchmark`
- `src/arxiv_archive/staging/import_boundary.py:validate_import_boundary_rehearsal`
- `src/arxiv_archive/staging/import_boundary.py:write_import_boundary_rehearsal_run`

**Unsafe-to-claim boundary**

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

**Next actions**

- Use fail-closed import rehearsal only after graph-readiness review passes.
- Do not write LadybugDB or production graph state from this readiness report.
