# ⚠️ HISTORICAL — pre-M105 (2026-06-04). This document predates the M105 onion completion. Updated information: see `doc/onion-layers.md`, `doc/adr/ADR-034`, `doc/MIGRATION.md`, and `.gsd/milestones/M105-269bqo/M105-269bqo-SUMMARY.md`.
# M030 Pipeline Module Inventory

Readable report rendered for M030 S02 T02 from the machine-readable inventory. This report records module ownership and evidence only; it does **not** claim source acquisition, parser readiness, chunk readiness, graph readiness, LadybugDB writes, or production import for the M030 requested refs.

## Scope

- Milestone: `M030-abwhdm`
- Slice: `S02` Code Module Inventory
- Task: `T02` Write module inventory markdown report
- Source inventory: `doc/architecture/m030_pipeline_module_inventory.json`
- Schema version: `m030-pipeline-module-inventory.v1`
- Inventory created by: `T01`
- Report path: `doc/architecture/m030_pipeline_module_inventory.md`

## Boundary Statement

- Behavior changed: `false`
- Runtime replay required: `false`
- Readiness claimed: `false`
- Notes: This inventory records ownership and evidence only. It does not register missing M030 refs, acquire sources, parse/chunk new articles, promote graph readiness, write LadybugDB, or attempt production import.

## Stage Coverage Summary

| Stage | Inventory row(s) | Required | Status |
|---|---|---:|---|
| URL intake | `m030_requested_ref_intake` | yes | Covered |
| Article catalog | `metadata_only_catalog_registration` | yes | Covered |
| Source acquisition | `mixed_source_capture_boundary` | yes | Covered |
| Loader evidence | `local_ingestion_loader_and_evidence_bridge` | yes | Covered |
| Parser/conversion | `conversion_quality_and_parser_boundary` | yes | Covered |
| Chunking | `pageindex_semantic_chunk_evidence` | yes | Covered |
| Graph-readiness review | `graph_readiness_export_and_independent_review` | yes | Covered |
| Graph import boundary | `fail_closed_import_contract_and_rehearsal` | yes | Covered |
| Cross-stage replay | `current_pipeline_and_end_to_end_replay` | no | Covered |

## GitNexus Evidence

1. Query: `URL intake article catalog source acquisition loader evidence parser conversion chunking graph readiness graph import`
   - Repo: `daily-archive`
   - Notable processes: `proc_272_register Register -> _default_arxiv_variants`, `proc_35_main Main -> Safe_relative_path`, `proc_15_verify Verify -> Rel`, `proc_23_verify Verify -> Rel`
   - Notable symbols: `scripts/register_m027_mixed_source_corpus.py:_article_record`, `scripts/register_m027_mixed_source_corpus.py:_default_arxiv_variants`, `scripts/replay_m027_current_pipeline_baseline.py:replay_baseline`, `scripts/verify_m027_current_pipeline_baseline.py:verify`, `scripts/verify_m027_conversion_quality_boundary.py:verify`
2. Query: `article_catalog source loader acquisition evidence`
   - Repo: `daily-archive`
   - Notable processes: `proc_272_register Register -> _default_arxiv_variants`, `proc_296_main Main -> _load_json`, `proc_36_main Main -> _looks_like_url`
   - Notable symbols: `scripts/verify_m025_article_catalog.py:build_catalog_readiness_artifacts`, `scripts/replay_m025_article_loader.py:replay_article`, `tests/test_m027_source_acquisition_boundary.py:test_cli_updates_all_six_selected_records_and_writes_metadata_only_artifacts`, `tests/test_m027_source_acquisition_boundary.py:test_cli_missing_index_row_fails_before_artifact_promotion`
3. Query: `graph_readiness_review graph import boundary LadybugDB import eligible`
   - Repo: `daily-archive`
   - Notable processes: `proc_129_render_reviewer_pack Render_reviewer_packet_markdown -> _escape_path`, `proc_107_render_bounded_chunk Render_bounded_chunk_repair_markdown -> _escape_path`
   - Notable symbols: `src/arxiv_archive/reviewer_packet_prototype.py:render_reviewer_packet_markdown`, `src/arxiv_archive/bounded_chunk_repair.py:render_bounded_chunk_repair_markdown`, `src/arxiv_archive/chunk_import_contract.py:ContractValidationResult.import_ready`, `tests/test_import_boundary_rehearsal.py:test_import_boundary_rehearsal_serializes_negative_candidate`
4. Query: `chunk repair stable id parser conversion pdf markdown article`
   - Repo: `daily-archive`
   - Notable processes: `proc_25_main Main -> Safe_relative_path`, `proc_35_main Main -> Safe_relative_path`
   - Notable symbols: `scripts/replay_m027_end_to_end_mixed_replay.py:replay_end_to_end`, `scripts/replay_m027_current_pipeline_baseline.py:replay_baseline`, `src/arxiv_archive/graph_readiness_export.py:_report`, `tests/test_m027_conversion_quality_boundary.py:test_local_conversion_classifies_abs_pdf_fallback_and_nature_body`

## Module Inventory

### 1. URL intake: `m030_requested_ref_intake`

- Status: `covered`

**Owner files**

- `data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json`
- `data/article_corpora/m029-pipeline-architecture-audit-v1/intake-report.md`
- `scripts/verify_m030_requested_ref_intake.py`

**Primary functions/classes**

- `scripts/verify_m030_requested_ref_intake.py:validate_selection`
- `scripts/verify_m030_requested_ref_intake.py:validate_report`
- `scripts/verify_m030_requested_ref_intake.py:validate_catalog_status`
- `scripts/verify_m030_requested_ref_intake.py:validate_m028_status`

**Inputs**

- Four human-requested URLs preserved in selection.json refs
- data/article_catalog/index.json
- data/article_corpora/m028-universal-loader-runtime-smoke-v1/selection.json

**Outputs/artifacts**

- Bounded selection JSON with normalized identities, catalog/prior-selection status, reachability metadata, and fail-closed unsafe_claims
- Human-readable intake report

**Tests/verifiers**

- `scripts/verify_m030_requested_ref_intake.py --validate-only`

**Evidence paths**

- `.gsd/milestones/M030-abwhdm/slices/S01/S01-SUMMARY.md`
- `data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json`
- `scripts/verify_m030_requested_ref_intake.py`

**Failure Modes**

- Malformed JSON raises validation error before success output.
- Catalog status drift against article_catalog/index.json is reported as M030_INTAKE_CATALOG_LINK.
- M028 linkage drift is reported as M030_INTAKE_M028_LINK.
- Unsafe parser/chunk/graph/source-acquisition claims are rejected when flags are not false.

**Load Profile**

Expected four refs; at 10x the first saturation is local JSON/report scanning. No network or subprocess load is introduced by validation.

**Negative Tests**

- No dedicated pytest file yet for M030 intake; S01 verifier itself checks malformed shape, count drift, catalog drift, M028 drift, and unsafe claims.

**Observability surfaces**

- Stable M030_INTAKE_* diagnostic codes
- Success line reports refs/cataloged/missing counts and fail-closed status

---

### 2. Article catalog: `metadata_only_catalog_registration`

- Status: `covered`

**Owner files**

- `scripts/register_m027_mixed_source_corpus.py`
- `data/article_catalog/index.json`
- `data/article_catalog/catalog.json`

**Primary functions/classes**

- `scripts/register_m027_mixed_source_corpus.py:ArticleSpec`
- `scripts/register_m027_mixed_source_corpus.py:_default_arxiv_strategy`
- `scripts/register_m027_mixed_source_corpus.py:_default_arxiv_variants`
- `scripts/register_m027_mixed_source_corpus.py:_article_record`
- `scripts/register_m027_mixed_source_corpus.py:_selection_payload`
- `scripts/register_m027_mixed_source_corpus.py:register`

**Inputs**

- Manual URL/catalog specs
- Existing article_catalog catalog/index JSON

**Outputs/artifacts**

- article_catalog/<source>/<topic>/<article>/article.json records
- article_catalog/index.json rows
- M027 metadata-only selection payload

**Tests/verifiers**

- `tests/test_m027_mixed_source_catalog.py`
- `tests/test_article_catalog_schema.py`
- `scripts/verify_m025_article_catalog.py`

**Evidence paths**

- `scripts/register_m027_mixed_source_corpus.py`
- `tests/test_article_catalog_schema.py`
- `tests/test_m027_mixed_source_catalog.py`

**Failure Modes**

- Duplicate refs, duplicate URLs, duplicate titles, unsafe article refs, missing titles, unsupported source_code, and malformed arXiv keys emit diagnostics before registration can be considered valid.
- Malformed JSON while merging catalog/index raises RuntimeError instead of silently overwriting state.
- Default article records are metadata-only and set source_artifact_captured/network_fetch_attempted false.

**Load Profile**

Catalog registration is file-based JSON mutation. At 10x selected articles, index merge/read/write and article JSON writes saturate before CPU; no network fetch is performed during registration.

**Negative Tests**

- `tests/test_article_catalog_schema.py::TestArticleSchemaV0001.test_selected_articles_follow_source_topic_article_hierarchy`
- `tests/test_article_catalog_schema.py::TestArticleSchemaV0001.test_arxiv_articles_capture_pdf_immediately_but_prefer_html_when_available`
- `tests/test_m027_mixed_source_catalog.py`

**Observability surfaces**

- Registration diagnostics include selection_id, article_ref, seed_url, fail_closed_safety_flags, and network_fetch_attempted=false

---

### 3. Source acquisition: `mixed_source_capture_boundary`

- Status: `covered`

**Owner files**

- `scripts/capture_m027_mixed_source_sources.py`
- `scripts/verify_m027_source_acquisition_boundary.py`
- `tests/test_m027_source_acquisition_boundary.py`

**Primary functions/classes**

- `scripts/capture_m027_mixed_source_sources.py:FetchResponse`
- `scripts/capture_m027_mixed_source_sources.py:default_fetcher`
- `scripts/capture_m027_mixed_source_sources.py:fixture_response_fetcher`
- `scripts/capture_m027_mixed_source_sources.py:target_path_for_variant`
- `scripts/capture_m027_mixed_source_sources.py:diagnostic_result`
- `scripts/verify_m027_source_acquisition_boundary.py:validate_captured_variant`

**Inputs**

- Selected catalog article records via index.json
- source_variants with URL, source_role, and expected local target
- Optional fixture response directory for offline tests

**Outputs/artifacts**

- Catalog-local source artifacts such as source/abs.html, source/original.pdf, source/article.html
- source-acquisition-summary.json
- source-acquisition-diagnostics.jsonl
- source-acquisition-report.md
- Updated article.json source variant capture metadata

**Tests/verifiers**

- `tests/test_m027_source_acquisition_boundary.py`
- `scripts/verify_m027_source_acquisition_boundary.py`

**Evidence paths**

- `scripts/capture_m027_mixed_source_sources.py`
- `scripts/verify_m027_source_acquisition_boundary.py`
- `tests/test_m027_source_acquisition_boundary.py`

**Failure Modes**

- Network timeout or urllib errors become failed/blocked diagnostics for the variant and do not create fallback captures.
- Missing fixture response raises FileNotFoundError and records blocked diagnostics in tests/offline replay.
- Empty response records empty_response and leaves target artifact absent.
- Unsafe catalog paths, duplicate index rows, and output traversal raise ValueError before artifact promotion.

**Load Profile**

Real M027 expected load is six articles and eleven variants. At 10x, HTTP requests and local byte writes saturate first; protection is fixed role-to-path mapping, 25s timeout, fixture injection for tests, atomic writes, bounded metadata artifacts, and fail-closed graph/import flags.

**Negative Tests**

- `tests/test_m027_source_acquisition_boundary.py::test_cli_missing_index_row_fails_before_artifact_promotion`
- `tests/test_m027_source_acquisition_boundary.py::test_cli_duplicate_or_unsafe_index_path_is_rejected`
- `tests/test_m027_source_acquisition_boundary.py::test_cli_fixture_failure_response_records_failed_diagnostic_without_fallback`
- `tests/test_m027_source_acquisition_boundary.py::test_cli_rejects_output_dir_traversal`

**Observability surfaces**

- Per-variant diagnostics with diagnostic_code, status, sha256, byte_size, media_type, and safety flags
- Summary counts for captured/blocked/failed

---

### 4. Loader evidence: `local_ingestion_loader_and_evidence_bridge`

- Status: `covered`

**Owner files**

- `src/arxiv_archive/full_text.py`
- `src/arxiv_archive/ingestion/loader.py`
- `src/arxiv_archive/article_evidence_bridge.py`
- `tests/test_full_text_ingestion.py`

**Primary functions/classes**

- `src/arxiv_archive/ingestion/loader.py:FullTextSource`
- `src/arxiv_archive/ingestion/loader.py:FullTextIngestionResult`
- `src/arxiv_archive/ingestion/loader.py:ArticleLoadSource`
- `src/arxiv_archive/ingestion/loader.py:ArticleLoadResult`
- `src/arxiv_archive/ingestion/loader.py:ingest_full_text`
- `src/arxiv_archive/ingestion/loader.py:load_article_source`
- `src/arxiv_archive/article_evidence_bridge.py:build_article_evidence_bundle_from_load_events`

**Inputs**

- Local markdown/text source paths
- Article load events or ArticleLoadResult records

**Outputs/artifacts**

- FullTextIngestionResult with extraction_mode, warnings, fallback_reason, quality, and provenance
- Article loader events/evidence bundles without raw text in metadata

**Tests/verifiers**

- `tests/test_full_text_ingestion.py`
- `tests/test_article_evidence_bridge.py`

**Evidence paths**

- `src/arxiv_archive/full_text.py`
- `src/arxiv_archive/ingestion/loader.py`
- `tests/test_full_text_ingestion.py`

**Failure Modes**

- Unsupported full_text source types raise ValueError before parsing.
- Missing source returns typed missing_source result with warning and source_missing fallback_reason.
- Empty source returns empty_source result with source_empty fallback_reason.
- Low-quality arXiv landing markdown is formalized as low_quality_source/no_substantive_body.

**Load Profile**

Loader reads local files into memory; at 10x the first saturation is filesystem throughput and text memory for large converted payloads. Protection is local-only source types, quality status, and metadata diagnostics rather than network/database calls.

**Negative Tests**

- `tests/test_full_text_ingestion.py::test_missing_source_returns_typed_failure_without_empty_silent_text`
- `tests/test_full_text_ingestion.py::test_empty_or_malformed_source_returns_explicit_warning`
- `tests/test_full_text_ingestion.py::test_low_quality_arxiv_landing_markdown_is_formalized`
- `tests/test_full_text_ingestion.py::test_rejects_unknown_source_type_before_parsing`

**Observability surfaces**

- FullTextQualityReport counters and warnings
- ArticleLoadResult duration_ms, warning_count, outcome, failure_reason, source_id, checksum

---

### 5. Parser/conversion: `conversion_quality_and_parser_boundary`

- Status: `covered`

**Owner files**

- `scripts/convert_m027_source_quality_boundary.py`
- `scripts/verify_m027_conversion_quality_boundary.py`
- `src/arxiv_archive/parsing/parser.py`
- `tests/test_m027_conversion_quality_boundary.py`
- `tests/test_page_index.py`

**Primary functions/classes**

- `scripts/convert_m027_source_quality_boundary.py:verify_source_bytes`
- `scripts/convert_m027_source_quality_boundary.py:converted_text_path`
- `scripts/verify_m027_conversion_quality_boundary.py:verify`
- `scripts/verify_m027_conversion_quality_boundary.py:validate_row_semantics`
- `src/arxiv_archive/parsing/parser.py:parse_article`
- `src/arxiv_archive/parsing/parser.py:_fallback_article`

**Inputs**

- source-acquisition-summary.json
- Captured HTML/PDF artifacts
- FullTextIngestionResult for parser boundary

**Outputs/artifacts**

- conversion-quality-summary.json
- conversion-quality-diagnostics.jsonl
- conversion-quality-report.md
- conversion-quality/<article>/<role>.txt payloads referenced by hash/size/path
- ParsedArticle/ParsedArticleElement records

**Tests/verifiers**

- `tests/test_m027_conversion_quality_boundary.py`
- `scripts/verify_m027_conversion_quality_boundary.py`
- `tests/test_page_index.py`

**Evidence paths**

- `scripts/convert_m027_source_quality_boundary.py`
- `scripts/verify_m027_conversion_quality_boundary.py`
- `src/arxiv_archive/parsing/parser.py`
- `tests/test_m027_conversion_quality_boundary.py`

**Failure Modes**

- Missing, unreadable, hash-mismatched, or non-captured source rows are blocked without parser_ready claims.
- Unsafe article_ref/local_path/converted_text_path are rejected.
- arXiv abs HTML is classified metadata_only; missing PDF fallback is a verifier failure.
- Metadata payload leakage and unsafe safety flags fail verification.
- Parser with no headings emits fallback full-text section and warning instead of silently dropping content.

**Load Profile**

Conversion currently bounds PDFs to MAX_PDF_PAGES=8 and MAX_TEXT_CHARS=80000. At 10x, PyMuPDF/BeautifulSoup parsing and local converted-text writes saturate first; protection is page/character caps, metadata-only handling for abs pages, deterministic payload paths, and hash verification.

**Negative Tests**

- `tests/test_m027_conversion_quality_boundary.py::test_unsafe_article_ref_or_local_path_is_blocked_without_conversion`
- `tests/test_m027_conversion_quality_boundary.py::test_missing_hash_mismatch_and_non_captured_rows_fail_closed`
- `tests/test_m027_conversion_quality_boundary.py::test_conversion_verifier_fails_on_unsafe_converted_text_path`
- `tests/test_m027_conversion_quality_boundary.py::test_conversion_verifier_fails_on_stale_source_and_converted_hashes`
- `tests/test_m027_conversion_quality_boundary.py::test_conversion_verifier_fails_on_metadata_payload_leakage`
- `tests/test_m027_conversion_quality_boundary.py::test_conversion_verifier_fails_when_arxiv_pdf_fallback_is_missing`
- `tests/test_m027_conversion_quality_boundary.py::test_conversion_verifier_fails_on_unsafe_safety_flags`
- `tests/test_page_index.py::test_parser_boundary_reports_fallback_before_indexing`

**Observability surfaces**

- diagnostic_code per conversion row
- quality status/counters and structure_counts
- source and converted payload hashes/byte sizes
- Failure Modes, Load Profile, and Negative Tests report sections required by verifier

---

### 6. Chunking: `pageindex_semantic_chunk_evidence`

- Status: `covered`

**Owner files**

- `src/arxiv_archive/page_index.py`
- `src/arxiv_archive/indexing/page_index.py`
- `src/arxiv_archive/evidence.py`
- `tests/test_evidence_paths.py`
- `tests/test_page_index.py`

**Primary functions/classes**

- `src/arxiv_archive/indexing/page_index.py:build_page_index`
- `src/arxiv_archive/indexing/page_index.py:build_page_index_from_parsed`
- `src/arxiv_archive/evidence.py:SemanticChunk`
- `src/arxiv_archive/evidence.py:EvidencePath`
- `src/arxiv_archive/evidence.py:build_semantic_chunks`
- `src/arxiv_archive/evidence.py:build_evidence_paths`
- `src/arxiv_archive/evidence.py:validate_evidence_path`

**Inputs**

- ParsedArticle from parser boundary
- PageIndexDocument

**Outputs/artifacts**

- PageIndexDocument/PageIndexNode hierarchy
- SemanticChunk records with section_text_v1 strategy
- EvidencePath records linking paper -> PageIndexNode -> SemanticChunk

**Tests/verifiers**

- `tests/test_evidence_paths.py`
- `tests/test_page_index.py`
- `scripts/replay_m027_end_to_end_mixed_replay.py`

**Evidence paths**

- `src/arxiv_archive/evidence.py`
- `src/arxiv_archive/indexing/page_index.py`
- `tests/test_evidence_paths.py`
- `scripts/replay_m027_end_to_end_mixed_replay.py`

**Failure Modes**

- Empty PageIndex sections emit validation warnings and no chunk instead of fake chunks.
- Missing or mismatched evidence links are reported by validate_evidence_path.
- Parser-ready zero-chunk variants are preserved as diagnostics in end-to-end replay and block import readiness.

**Load Profile**

Chunking is in-memory over PageIndex nodes. At 10x, memory/CPU for section traversal and evidence path construction saturate before external dependencies; protection is deterministic one-pass section_text_v1 chunking and no graph/database writers.

**Negative Tests**

- `tests/test_evidence_paths.py::test_skips_empty_root_and_reports_empty_section_diagnostic`
- `tests/test_evidence_paths.py::test_evidence_path_validation_reports_missing_and_mismatched_links`
- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_preserves_parser_ready_zero_chunk_diagnostic`

**Observability surfaces**

- PageIndex validation_warnings
- SemanticChunk provenance fields
- EvidencePath validation_warnings
- Replay chunk_count and evidence_path_count

---

### 7. Graph-readiness review: `graph_readiness_export_and_independent_review`

- Status: `covered`

**Owner files**

- `src/arxiv_archive/graph_readiness_export.py`
- `src/arxiv_archive/graph_readiness_review.py`
- `tests/test_graph_readiness_export.py`
- `tests/test_graph_readiness_review.py`

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

**Inputs**

- Corpus manifest documents with expected_full_text_path
- Local full_text.md and optional full_text.method files
- NormalizedPaperPackage outputs for review selection

**Outputs/artifacts**

- graph-readiness-events.jsonl
- graph-readiness-summary.json
- review/*.md bounded reviewer bundles
- independent-review-events.jsonl
- independent-review-summary.md

**Tests/verifiers**

- `tests/test_graph_readiness_export.py`
- `tests/test_graph_readiness_review.py`
- uv run python -m arxiv_archive.graph_readiness_review --review-dir <review-dir> --events <events.jsonl> --validate-only --require-completed-review

**Evidence paths**

- `src/arxiv_archive/graph_readiness_export.py`
- `src/arxiv_archive/graph_readiness_review.py`
- `tests/test_graph_readiness_export.py`
- `tests/test_graph_readiness_review.py`

**Failure Modes**

- Low-quality sources are rejected without chunks.
- Deprecated conversion methods mark packages repair_required.
- Review bundles are bounded snippets and events omit article body text.
- Completed-review validator catches placeholders and missing output_contract_completed=true verdict events.

**Load Profile**

Graph-readiness export builds normalized packages per manifest document. At 10x, local full_text reads and in-memory chunk/route classification saturate first; protection is redacted summaries/events, bounded snippet sizes, route blockers, and validate-only independent review before eligibility promotion.

**Negative Tests**

- `tests/test_graph_readiness_export.py::test_low_quality_source_is_rejected_without_chunks`
- `tests/test_graph_readiness_export.py::test_deprecated_pymupdf_method_marks_package_repair_required`
- `tests/test_graph_readiness_review.py::test_generated_summary_states_review_is_required_before_eligibility`
- `tests/test_graph_readiness_review.py::test_validate_review_artifacts_allows_generated_contracts_before_completion`

**Observability surfaces**

- Graph readiness route events
- Summary package counts/states
- Independent review request and summary events
- Stable review artifact validation diagnostics

---

### 8. Graph import boundary: `fail_closed_import_contract_and_rehearsal`

- Status: `covered`

**Owner files**

- `src/arxiv_archive/chunk_import_contract.py`
- `src/arxiv_archive/import_boundary_rehearsal.py`
- `src/arxiv_archive/staging/import_boundary.py`
- `tests/test_import_ready_contract.py`
- `tests/test_import_boundary_rehearsal.py`

**Primary functions/classes**

- `src/arxiv_archive/chunk_import_contract.py:validate_import_ready_package`
- `src/arxiv_archive/chunk_import_contract.py:validation_to_dict`
- `src/arxiv_archive/chunk_import_contract.py:ContractValidationResult.import_ready`
- `src/arxiv_archive/staging/import_boundary.py:ImportCandidate`
- `src/arxiv_archive/staging/import_boundary.py:ImportBoundaryRehearsal`
- `src/arxiv_archive/staging/import_boundary.py:build_import_boundary_rehearsal_from_benchmark`
- `src/arxiv_archive/staging/import_boundary.py:validate_import_boundary_rehearsal`
- `src/arxiv_archive/staging/import_boundary.py:write_import_boundary_rehearsal_run`

**Inputs**

- Import-ready chunk package dictionaries
- Chunking benchmark summary/diagnostics for negative rehearsal

**Outputs/artifacts**

- ContractValidationResult/validation_to_dict metadata
- ImportBoundaryRehearsal contract with accepted_count=0 for refused candidates
- Refusal counts and remediation hints

**Tests/verifiers**

- `tests/test_import_ready_contract.py`
- `tests/test_import_boundary_rehearsal.py`

**Evidence paths**

- `src/arxiv_archive/chunk_import_contract.py`
- `src/arxiv_archive/staging/import_boundary.py`
- `tests/test_import_boundary_rehearsal.py`

**Failure Modes**

- Schema/header mismatches, missing package sections, invalid routes/states, missing evidence links, and package diagnostic mismatches produce refusing diagnostics.
- Raw text, embeddings, vectors, secrets, and optimizer traces are forbidden recursively.
- Candidates that allow trusted_kg_import while not import_eligible are rejected.
- production_import_attempted and ladybugdb_written true are unsafe write-flag failures.

**Load Profile**

Import-boundary validation is pure in-memory dictionary traversal. At 10x, candidate/chunk count traversal saturates CPU/memory; protection is no graph/database calls, redacted contracts, refusal counts, and explicit excluded_uses for trusted import.

**Negative Tests**

- `tests/test_import_boundary_rehearsal.py::test_validate_import_boundary_rehearsal_rejects_count_mismatch`
- `tests/test_import_boundary_rehearsal.py::test_validate_import_boundary_rehearsal_rejects_positive_import_for_refused_candidate`
- `tests/test_import_boundary_rehearsal.py::test_validate_import_boundary_rehearsal_rejects_unsafe_write_flags`
- `tests/test_import_boundary_rehearsal.py::test_validate_import_boundary_rehearsal_rejects_nested_forbidden_fields_without_values`
- `tests/test_import_ready_contract.py`

**Observability surfaces**

- refusal_counts by stable reason
- diagnostics with object_id/object_type/route and blocks_import
- import_ready=false, production_import_attempted=false, ladybugdb_written=false metadata

---

### 9. Cross-stage replay: `current_pipeline_and_end_to_end_replay`

- Status: `covered`

**Owner files**

- `scripts/replay_m027_current_pipeline_baseline.py`
- `scripts/verify_m027_current_pipeline_baseline.py`
- `scripts/replay_m027_end_to_end_mixed_replay.py`
- `tests/test_m027_current_pipeline_baseline.py`
- `tests/test_m027_end_to_end_mixed_replay.py`

**Primary functions/classes**

- `scripts/replay_m027_current_pipeline_baseline.py:replay_baseline`
- `scripts/replay_m027_current_pipeline_baseline.py:run_current_pipeline`
- `scripts/replay_m027_end_to_end_mixed_replay.py:replay_end_to_end`
- `scripts/replay_m027_end_to_end_mixed_replay.py:run_boundaries`
- `scripts/replay_m027_end_to_end_mixed_replay.py:build_readiness_decision`

**Inputs**

- S03 conversion-quality-summary.json
- S04 current-pipeline baseline summary/diagnostics
- Converted payload files verified by hash/size

**Outputs/artifacts**

- baseline.json/replay.json per-article artifacts
- summary/diagnostics/events/report artifacts
- readiness decision with ready_for_import=false when blockers remain

**Tests/verifiers**

- `tests/test_m027_current_pipeline_baseline.py`
- `tests/test_m027_end_to_end_mixed_replay.py`
- `scripts/verify_m027_current_pipeline_baseline.py`

**Evidence paths**

- `scripts/replay_m027_current_pipeline_baseline.py`
- `scripts/replay_m027_end_to_end_mixed_replay.py`
- `tests/test_m027_end_to_end_mixed_replay.py`

**Failure Modes**

- --no-network is required; missing/malformed S03/S04 JSON, stale hashes, unsafe paths, and missing converted payloads raise replay errors before readiness claims.
- Metadata-only variants are skipped without payload reads.
- Parser-ready zero chunks are preserved as blockers.
- Raw text/HTML/PDF/key leakage guard rejects unsafe output metadata.

**Load Profile**

Expected real corpus is six articles and eleven variants. At 10x, filesystem reads/writes and in-memory loader/parser/PageIndex/chunk/evidence construction over converted text payloads saturate first; protection is one-variant-at-a-time replay, bounded S03 payloads, redacted per-article artifacts, and no network/database/graph writers.

**Negative Tests**

- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_rejects_converted_payload_hash_mismatch`
- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_preserves_parser_ready_zero_chunk_diagnostic`
- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_skips_metadata_only_without_payload`
- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_rejects_unsafe_output_dir`
- `tests/test_m027_end_to_end_mixed_replay.py::test_metadata_outputs_are_redacted`
- `tests/test_m027_end_to_end_mixed_replay.py::test_redaction_guard_rejects_payload_keys_and_snippets`

**Observability surfaces**

- input_artifacts/output_artifacts with hashes
- diagnostic_counts and baseline_comparison_counts
- events replay_started/variant_replayed/replay_completed
- failure_modes/load_profile embedded in summaries and reports

---

## Data Artifacts

- `data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json`
- `data/article_catalog/index.json`
- `data/article_corpora/m028-universal-loader-runtime-smoke-v1/selection.json`
- `data/article_corpora/m027-mixed-source-corpus-v1/source-acquisition-summary.json`
- `data/article_corpora/m027-mixed-source-corpus-v1/conversion-quality-summary.json`

## Unknown or Stale Index Areas

- {'area': 'Direct production LadybugDB graph writer for article chunks', 'status': 'not identified in this inventory as an enabled article pipeline stage', 'safe_interpretation': 'Current graph/import boundaries are validate-only/rehearsal contracts with ladybugdb_written=false and production_import_attempted=false. Do not infer production KG import readiness.'}
- {'area': 'M030-specific source acquisition for the three missing/current requested refs', 'status': 'not implemented by this task', 'safe_interpretation': 'Use catalog registration plus acquisition/conversion replay in downstream slices before claiming coverage.'}

## Quality Gate Notes

| Gate | Verdict | Evidence summary |
|---|---|---|
| Q5 Failure Modes | `addressed` | Each module row lists external dependencies and fail-closed/malformed/timeout/missing-path behavior. |
| Q6 Load Profile | `addressed` | Each module row identifies likely 10x saturation and protections. |
| Q7 Negative Tests | `addressed` | Each module row names test files/cases or, for M030 intake, the local verifier checks that currently cover negative surfaces. |

## Verification Contract

{'task_verification': 'uv run python -m json.tool doc/architecture/m030_pipeline_module_inventory.json', 'slice_health_signal': 'A future validator should require non-empty modules, all required_stages present in stage_coverage, evidence_paths on every row, and graph_import_boundary represented fail-closed.'}

## Reader Checklist

- Every required stage has at least one module row.
- Every module row has owner files, functions/classes, artifacts, evidence paths, tests/verifiers, failure modes, load profile, negative tests, and observability surfaces.
- Graph-readiness and graph-import rows are inventory evidence only and remain fail-closed; this report does not promote article refs to graph import readiness.
