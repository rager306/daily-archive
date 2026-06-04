# M030 Pipeline Module Inventory

Static inventory for M030 S02 T01. This report records module ownership and evidence only; it does **not** claim source acquisition, parser readiness, chunk readiness, graph readiness, LadybugDB writes, or production import for the M030 requested refs.

## Scope

- Milestone: `M030-abwhdm`
- Slice: `S02` Code Module Inventory
- Task: `T01` Discover pipeline modules and evidence sources
- Machine-readable inventory: `doc/architecture/m030_pipeline_module_inventory.json`
- S01 intake baseline consumed: `data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json`

## GitNexus Evidence

GitNexus queries used for discovery:

1. `URL intake article catalog source acquisition loader evidence parser conversion chunking graph readiness graph import`
   - Notable processes: `proc_272_register`, `proc_35_main`, `proc_15_verify`, `proc_23_verify`
   - Notable symbols: `register_m027_mixed_source_corpus.py:_article_record`, `_default_arxiv_variants`, `replay_m027_current_pipeline_baseline.py:replay_baseline`, `verify_m027_conversion_quality_boundary.py:verify`
2. `article_catalog source loader acquisition evidence`
   - Notable symbols: `verify_m025_article_catalog.py:build_catalog_readiness_artifacts`, `replay_m025_article_loader.py:replay_article`, `tests/test_m027_source_acquisition_boundary.py`
3. `graph_readiness_review graph import boundary LadybugDB import eligible`
   - Notable symbols: `graph_readiness_review.py`, `chunk_import_contract.py:ContractValidationResult.import_ready`, `tests/test_import_boundary_rehearsal.py`
4. `chunk repair stable id parser conversion pdf markdown article`
   - Notable symbols: `replay_m027_end_to_end_mixed_replay.py:replay_end_to_end`, `graph_readiness_export.py:_report`, `tests/test_m027_conversion_quality_boundary.py`

## Stage Coverage Summary

| Stage | Inventory row(s) | Status |
|---|---|---|
| URL intake | `m030_requested_ref_intake` | Covered |
| Article catalog | `metadata_only_catalog_registration` | Covered |
| Source acquisition | `mixed_source_capture_boundary` | Covered |
| Loader evidence | `local_ingestion_loader_and_evidence_bridge` | Covered |
| Parser/conversion | `conversion_quality_and_parser_boundary` | Covered |
| Chunking | `pageindex_semantic_chunk_evidence` | Covered |
| Graph-readiness review | `graph_readiness_export_and_independent_review` | Covered |
| Graph import boundary | `fail_closed_import_contract_and_rehearsal` | Covered |
| Cross-stage replay | `current_pipeline_and_end_to_end_replay` | Covered as integration evidence |

## Module Rows

### 1. URL intake: `m030_requested_ref_intake`

**Owner files**

- `data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json`
- `data/article_corpora/m029-pipeline-architecture-audit-v1/intake-report.md`
- `scripts/verify_m030_requested_ref_intake.py`

**Primary functions/classes**

- `validate_selection`
- `validate_report`
- `validate_catalog_status`
- `validate_m028_status`

**Inputs**

- Four user-requested URLs preserved in the M029/M030 intake selection.
- `data/article_catalog/index.json`
- `data/article_corpora/m028-universal-loader-runtime-smoke-v1/selection.json`

**Outputs**

- Bounded selection JSON with normalized identities, catalog/prior-selection status, reachability metadata, and fail-closed `unsafe_claims`.
- Human-readable intake report.

**Tests/verifiers**

- `scripts/verify_m030_requested_ref_intake.py --validate-only`

**Failure Modes**

- Malformed JSON or wrong top-level shape fails validation.
- Catalog drift is reported as `M030_INTAKE_CATALOG_LINK`.
- M028 prior-selection drift is reported as `M030_INTAKE_M028_LINK`.
- Unsafe source/parser/chunk/graph claims are rejected unless all expected flags remain false.

**Load Profile**

Expected load is four refs. At 10x, local JSON/report scanning saturates before CPU; there is no network or subprocess load in the verifier.

**Negative Tests**

No dedicated pytest file exists yet for the M030 intake verifier. Current negative coverage is encoded in `scripts/verify_m030_requested_ref_intake.py` itself: shape/count drift, catalog drift, M028 drift, and unsafe claims.

---

### 2. Article catalog: `metadata_only_catalog_registration`

**Owner files**

- `scripts/register_m027_mixed_source_corpus.py`
- `data/article_catalog/index.json`
- `data/article_catalog/catalog.json`

**Primary functions/classes**

- `ArticleSpec`
- `_default_arxiv_strategy`
- `_default_arxiv_variants`
- `_article_record`
- `_selection_payload`
- `register`

**Inputs**

- Manual URL/catalog specs.
- Existing article catalog/index JSON.

**Outputs**

- `article_catalog/<source>/<topic>/<article>/article.json` metadata records.
- `article_catalog/index.json` rows.
- M027 metadata-only selection payload.

**Tests/verifiers**

- `tests/test_m027_mixed_source_catalog.py`
- `tests/test_article_catalog_schema.py`
- `scripts/verify_m025_article_catalog.py`

**Failure Modes**

- Duplicate refs/URLs/titles, unsafe article refs, unsupported sources, and malformed arXiv keys emit diagnostics before registration can be considered valid.
- Malformed catalog/index JSON raises `RuntimeError` instead of silently overwriting state.
- Default records are metadata-only and keep `source_artifact_captured=false` and `network_fetch_attempted=false`.

**Load Profile**

At 10x selected articles, JSON index merge/read/write and article JSON writes saturate before CPU. Registration performs no network fetch.

**Negative Tests**

- `tests/test_article_catalog_schema.py::TestArticleSchemaV0001.test_selected_articles_follow_source_topic_article_hierarchy`
- `tests/test_article_catalog_schema.py::TestArticleSchemaV0001.test_arxiv_articles_capture_pdf_immediately_but_prefer_html_when_available`
- `tests/test_m027_mixed_source_catalog.py`

---

### 3. Source acquisition: `mixed_source_capture_boundary`

**Owner files**

- `scripts/capture_m027_mixed_source_sources.py`
- `scripts/verify_m027_source_acquisition_boundary.py`
- `tests/test_m027_source_acquisition_boundary.py`

**Primary functions/classes**

- `FetchResponse`
- `default_fetcher`
- `fixture_response_fetcher`
- `target_path_for_variant`
- `diagnostic_result`
- `validate_captured_variant`

**Inputs**

- Selected catalog article records through `index.json`.
- `source_variants` with URL, role, and target path.
- Optional fixture response directory for tests/offline replay.

**Outputs**

- Catalog-local source artifacts such as `source/abs.html`, `source/original.pdf`, and `source/article.html`.
- `source-acquisition-summary.json`
- `source-acquisition-diagnostics.jsonl`
- `source-acquisition-report.md`
- Updated article JSON capture metadata.

**Tests/verifiers**

- `tests/test_m027_source_acquisition_boundary.py`
- `scripts/verify_m027_source_acquisition_boundary.py`

**Failure Modes**

- Network timeout/urllib errors become failed or blocked variant diagnostics and do not create fallback captures.
- Missing fixture response raises `FileNotFoundError` and records blocked diagnostics in tests/offline replay.
- Empty response records `empty_response` and leaves target artifact absent.
- Unsafe catalog paths, duplicate index rows, and output traversal raise `ValueError` before artifact promotion.

**Load Profile**

The real M027 load is six articles and eleven variants. At 10x, HTTP requests and local byte writes saturate first. Protections include fixed role-to-path mapping, a 25s timeout, fixture injection for tests, atomic writes, bounded metadata artifacts, and fail-closed graph/import flags.

**Negative Tests**

- `test_cli_missing_index_row_fails_before_artifact_promotion`
- `test_cli_duplicate_or_unsafe_index_path_is_rejected`
- `test_cli_fixture_failure_response_records_failed_diagnostic_without_fallback`
- `test_cli_rejects_output_dir_traversal`

---

### 4. Loader evidence: `local_ingestion_loader_and_evidence_bridge`

**Owner files**

- `src/arxiv_archive/full_text.py`
- `src/arxiv_archive/ingestion/loader.py`
- `src/arxiv_archive/article_evidence_bridge.py`
- `tests/test_full_text_ingestion.py`

**Primary functions/classes**

- `FullTextSource`
- `FullTextIngestionResult`
- `ArticleLoadSource`
- `ArticleLoadResult`
- `ingest_full_text`
- `load_article_source`
- `build_article_evidence_bundle_from_load_events`

**Inputs**

- Local markdown/text source paths.
- Article load events or `ArticleLoadResult` records.

**Outputs**

- `FullTextIngestionResult` with extraction mode, warnings, fallback reason, quality, and provenance.
- Article loader events/evidence bundles without raw text in metadata.

**Tests/verifiers**

- `tests/test_full_text_ingestion.py`
- `tests/test_article_evidence_bridge.py`

**Failure Modes**

- Unsupported full-text source types raise `ValueError` before parsing.
- Missing source returns typed `missing_source` with warning and `source_missing` fallback reason.
- Empty source returns `empty_source` with `source_empty` fallback reason.
- Low-quality arXiv landing markdown is formalized as `low_quality_source/no_substantive_body`.

**Load Profile**

Loader reads local files into memory. At 10x, filesystem throughput and text memory for large converted payloads saturate first. Protections are local-only source types, quality status, and metadata diagnostics rather than network/database calls.

**Negative Tests**

- `test_missing_source_returns_typed_failure_without_empty_silent_text`
- `test_empty_or_malformed_source_returns_explicit_warning`
- `test_low_quality_arxiv_landing_markdown_is_formalized`
- `test_rejects_unknown_source_type_before_parsing`

---

### 5. Parser/conversion: `conversion_quality_and_parser_boundary`

**Owner files**

- `scripts/convert_m027_source_quality_boundary.py`
- `scripts/verify_m027_conversion_quality_boundary.py`
- `src/arxiv_archive/parsing/parser.py`
- `tests/test_m027_conversion_quality_boundary.py`
- `tests/test_page_index.py`

**Primary functions/classes**

- `verify_source_bytes`
- `converted_text_path`
- `verify`
- `validate_row_semantics`
- `parse_article`
- `_fallback_article`

**Inputs**

- `source-acquisition-summary.json`
- Captured HTML/PDF artifacts.
- `FullTextIngestionResult` for the parser boundary.

**Outputs**

- `conversion-quality-summary.json`
- `conversion-quality-diagnostics.jsonl`
- `conversion-quality-report.md`
- Converted text payloads referenced by hash/size/path.
- `ParsedArticle` and `ParsedArticleElement` records.

**Tests/verifiers**

- `tests/test_m027_conversion_quality_boundary.py`
- `scripts/verify_m027_conversion_quality_boundary.py`
- `tests/test_page_index.py`

**Failure Modes**

- Missing, unreadable, hash-mismatched, or non-captured source rows are blocked without parser-ready claims.
- Unsafe `article_ref`, `local_path`, and `converted_text_path` are rejected.
- arXiv abs HTML is classified metadata-only; missing PDF fallback is a verifier failure.
- Metadata payload leakage and unsafe safety flags fail verification.
- Parser input with no headings emits fallback full-text section and warning instead of silently dropping content.

**Load Profile**

Conversion bounds PDFs to `MAX_PDF_PAGES=8` and text to `MAX_TEXT_CHARS=80000`. At 10x, PyMuPDF/BeautifulSoup parsing and converted-text writes saturate first. Protections are page/character caps, metadata-only handling for abs pages, deterministic payload paths, and hash verification.

**Negative Tests**

- `test_unsafe_article_ref_or_local_path_is_blocked_without_conversion`
- `test_missing_hash_mismatch_and_non_captured_rows_fail_closed`
- `test_conversion_verifier_fails_on_unsafe_converted_text_path`
- `test_conversion_verifier_fails_on_stale_source_and_converted_hashes`
- `test_conversion_verifier_fails_on_metadata_payload_leakage`
- `test_conversion_verifier_fails_when_arxiv_pdf_fallback_is_missing`
- `test_conversion_verifier_fails_on_unsafe_safety_flags`
- `tests/test_page_index.py::test_parser_boundary_reports_fallback_before_indexing`

---

### 6. Chunking: `pageindex_semantic_chunk_evidence`

**Owner files**

- `src/arxiv_archive/page_index.py`
- `src/arxiv_archive/indexing/page_index.py`
- `src/arxiv_archive/evidence.py`
- `tests/test_evidence_paths.py`
- `tests/test_page_index.py`

**Primary functions/classes**

- `build_page_index`
- `build_page_index_from_parsed`
- `SemanticChunk`
- `EvidencePath`
- `build_semantic_chunks`
- `build_evidence_paths`
- `validate_evidence_path`

**Inputs**

- `ParsedArticle` from parser boundary.
- `PageIndexDocument`.

**Outputs**

- `PageIndexDocument` / `PageIndexNode` hierarchy.
- `SemanticChunk` records with `section_text_v1` strategy.
- `EvidencePath` records linking paper -> PageIndexNode -> SemanticChunk.

**Tests/verifiers**

- `tests/test_evidence_paths.py`
- `tests/test_page_index.py`
- `scripts/replay_m027_end_to_end_mixed_replay.py`

**Failure Modes**

- Empty PageIndex sections emit validation warnings and no chunk instead of fake chunks.
- Missing/mismatched evidence links are reported by `validate_evidence_path`.
- Parser-ready zero-chunk variants are preserved as replay diagnostics and block import readiness.

**Load Profile**

Chunking is in-memory over PageIndex nodes. At 10x, memory/CPU for section traversal and evidence path construction saturate before external dependencies. Protections are deterministic one-pass `section_text_v1` chunking and no graph/database writers.

**Negative Tests**

- `test_skips_empty_root_and_reports_empty_section_diagnostic`
- `test_evidence_path_validation_reports_missing_and_mismatched_links`
- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_preserves_parser_ready_zero_chunk_diagnostic`

---

### 7. Graph-readiness review: `graph_readiness_export_and_independent_review`

**Owner files**

- `src/arxiv_archive/graph_readiness_export.py`
- `src/arxiv_archive/graph_readiness_review.py`
- `tests/test_graph_readiness_export.py`
- `tests/test_graph_readiness_review.py`

**Primary functions/classes**

- `export_corpus`
- `build_package_from_manifest_document`
- `_graph_ready_chunks`
- `_evidence_path_refs`
- `_report`
- `generate_review_bundles`
- `select_review_papers`
- `render_review_bundle`
- `validate_review_artifacts`

**Inputs**

- Corpus manifest documents with `expected_full_text_path`.
- Local `full_text.md` and optional `full_text.method` files.
- `NormalizedPaperPackage` outputs for review selection.

**Outputs**

- `graph-readiness-events.jsonl`
- `graph-readiness-summary.json`
- Bounded review Markdown bundles.
- `independent-review-events.jsonl`
- `independent-review-summary.md`

**Tests/verifiers**

- `tests/test_graph_readiness_export.py`
- `tests/test_graph_readiness_review.py`
- Review artifact post-check: `uv run python -m arxiv_archive.graph_readiness_review --review-dir <review-dir> --events <events.jsonl> --validate-only --require-completed-review`

**Failure Modes**

- Low-quality sources are rejected without chunks.
- Deprecated conversion methods mark packages `repair_required`.
- Review bundles are bounded snippets and events omit article body text.
- Completed-review validator catches placeholders and missing `output_contract_completed=true` verdict events.

**Load Profile**

Graph-readiness export builds normalized packages per manifest document. At 10x, local full-text reads and in-memory chunk/route classification saturate first. Protections are redacted summaries/events, bounded snippet sizes, route blockers, and validate-only independent review before eligibility promotion.

**Negative Tests**

- `tests/test_graph_readiness_export.py::test_low_quality_source_is_rejected_without_chunks`
- `tests/test_graph_readiness_export.py::test_deprecated_pymupdf_method_marks_package_repair_required`
- `tests/test_graph_readiness_review.py::test_generated_summary_states_review_is_required_before_eligibility`
- `tests/test_graph_readiness_review.py::test_validate_review_artifacts_allows_generated_contracts_before_completion`

---

### 8. Graph import boundary: `fail_closed_import_contract_and_rehearsal`

**Owner files**

- `src/arxiv_archive/chunk_import_contract.py`
- `src/arxiv_archive/import_boundary_rehearsal.py`
- `src/arxiv_archive/staging/import_boundary.py`
- `tests/test_import_ready_contract.py`
- `tests/test_import_boundary_rehearsal.py`

**Primary functions/classes**

- `validate_import_ready_package`
- `validation_to_dict`
- `ContractValidationResult.import_ready`
- `ImportCandidate`
- `ImportBoundaryRehearsal`
- `build_import_boundary_rehearsal_from_benchmark`
- `validate_import_boundary_rehearsal`
- `write_import_boundary_rehearsal_run`

**Inputs**

- Import-ready chunk package dictionaries.
- Chunking benchmark summary/diagnostics for negative rehearsal.

**Outputs**

- `ContractValidationResult` / `validation_to_dict` metadata.
- `ImportBoundaryRehearsal` contract with `accepted_count=0` for refused candidates.
- Refusal counts and remediation hints.

**Tests/verifiers**

- `tests/test_import_ready_contract.py`
- `tests/test_import_boundary_rehearsal.py`

**Failure Modes**

- Schema/header mismatches, missing sections, invalid routes/states, missing evidence links, and package diagnostic mismatches produce refusing diagnostics.
- Raw text, embeddings, vectors, secrets, and optimizer traces are forbidden recursively.
- Candidates that allow `trusted_kg_import` while not import-eligible are rejected.
- `production_import_attempted=true` and `ladybugdb_written=true` are unsafe write-flag failures.

**Load Profile**

Import-boundary validation is pure in-memory dictionary traversal. At 10x, candidate/chunk count traversal saturates CPU/memory. Protections are no graph/database calls, redacted contracts, refusal counts, and explicit `excluded_uses` for trusted import.

**Negative Tests**

- `test_validate_import_boundary_rehearsal_rejects_count_mismatch`
- `test_validate_import_boundary_rehearsal_rejects_positive_import_for_refused_candidate`
- `test_validate_import_boundary_rehearsal_rejects_unsafe_write_flags`
- `test_validate_import_boundary_rehearsal_rejects_nested_forbidden_fields_without_values`
- `tests/test_import_ready_contract.py`

---

### 9. Cross-stage replay: `current_pipeline_and_end_to_end_replay`

**Owner files**

- `scripts/replay_m027_current_pipeline_baseline.py`
- `scripts/verify_m027_current_pipeline_baseline.py`
- `scripts/replay_m027_end_to_end_mixed_replay.py`
- `tests/test_m027_current_pipeline_baseline.py`
- `tests/test_m027_end_to_end_mixed_replay.py`

**Primary functions/classes**

- `replay_baseline`
- `run_current_pipeline`
- `replay_end_to_end`
- `run_boundaries`
- `build_readiness_decision`

**Inputs**

- S03 `conversion-quality-summary.json`.
- S04 current-pipeline baseline summary/diagnostics.
- Converted payload files verified by hash/size.

**Outputs**

- Per-article `baseline.json` / `replay.json` artifacts.
- Summary, diagnostics, events, and report artifacts.
- Readiness decision with `ready_for_import=false` when blockers remain.

**Tests/verifiers**

- `tests/test_m027_current_pipeline_baseline.py`
- `tests/test_m027_end_to_end_mixed_replay.py`
- `scripts/verify_m027_current_pipeline_baseline.py`

**Failure Modes**

- `--no-network` is required.
- Missing/malformed S03/S04 JSON, stale hashes, unsafe paths, and missing converted payloads raise replay errors before readiness claims.
- Metadata-only variants are skipped without payload reads.
- Parser-ready zero chunks are preserved as blockers.
- Raw text/HTML/PDF/key leakage guard rejects unsafe output metadata.

**Load Profile**

Expected real corpus is six articles and eleven variants. At 10x, filesystem reads/writes and in-memory loader/parser/PageIndex/chunk/evidence construction over converted text payloads saturate first. Protections are one-variant-at-a-time replay, bounded S03 payloads, redacted per-article artifacts, and no network/database/graph writers.

**Negative Tests**

- `test_replay_rejects_converted_payload_hash_mismatch`
- `test_replay_preserves_parser_ready_zero_chunk_diagnostic`
- `test_replay_skips_metadata_only_without_payload`
- `test_replay_rejects_unsafe_output_dir`
- `test_metadata_outputs_are_redacted`
- `test_redaction_guard_rejects_payload_keys_and_snippets`

## Data Artifacts

| Path | Role | Notes |
|---|---|---|
| `data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json` | M030 requested-ref intake baseline | All unsafe claims false |
| `data/article_catalog/index.json` | Article catalog index | Consumed by intake/catalog/acquisition checks |
| `data/article_corpora/m028-universal-loader-runtime-smoke-v1/selection.json` | Prior loader-smoke selection | Used for M030 intake linkage |
| `data/article_corpora/m027-mixed-source-corpus-v1/source-acquisition-summary.json` | M027 acquisition handoff | Expected for replay lineage; not required for this static inventory verification |
| `data/article_corpora/m027-mixed-source-corpus-v1/conversion-quality-summary.json` | M027 conversion handoff | Expected for replay lineage; not required for this static inventory verification |

## Unknown or Stale-Index Areas

- A direct production LadybugDB graph writer for article chunks was not identified as an enabled stage in this inventory. Safe interpretation: current graph/import boundaries are validate-only or negative-rehearsal contracts with `ladybugdb_written=false` and `production_import_attempted=false`.
- M030-specific source acquisition for requested refs is not implemented by this task. Downstream work must register missing catalog records and replay acquisition/conversion before claiming coverage.

## Failure Modes

Q5 is addressed by the module rows above. External dependencies are limited to local files, optional HTTP fetches in the source acquisition boundary, optional fixture response files, and local subprocess/interpreter imports in replay/verifier paths. The inventory itself performs no runtime acquisition and bubbles JSON/file errors through standard command failures.

## Load Profile

Q6 is addressed by the module rows above. Across the pipeline, the first 10x saturation points are HTTP fetches and byte writes for acquisition, PyMuPDF/BeautifulSoup conversion, local file reads/writes, and in-memory parser/PageIndex/chunk/evidence traversal. Existing protections are timeouts, fixture injection, page/character caps, one-variant-at-a-time replay, redacted artifacts, and no graph/database writers.

## Negative Tests

Q7 is addressed by the module rows above. The strongest negative coverage is in:

- `tests/test_m027_source_acquisition_boundary.py`
- `tests/test_m027_conversion_quality_boundary.py`
- `tests/test_full_text_ingestion.py`
- `tests/test_evidence_paths.py`
- `tests/test_graph_readiness_export.py`
- `tests/test_graph_readiness_review.py`
- `tests/test_import_boundary_rehearsal.py`
- `tests/test_m027_end_to_end_mixed_replay.py`

## Verification

Task verification command:

```bash
uv run python -m json.tool doc/architecture/m030_pipeline_module_inventory.json
```

Slice-level validator is planned for T03; this T01 inventory includes the required data for that validator: non-empty modules, stage coverage, evidence paths on every row, and explicit fail-closed graph import boundary representation.
