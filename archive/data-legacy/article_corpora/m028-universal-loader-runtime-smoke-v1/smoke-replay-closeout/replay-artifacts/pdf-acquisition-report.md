# M028 S03 PDF Acquisition Diagnostics

Metadata-only diagnostics for the accepted URL refs. This report records existing local PDF verification and typed terminal non-acquired/not-applicable reasons; it does not fetch URLs or serialize article/PDF bytes.

## Scope
- URL refs: 21
- Normalized identities: 20
- Duplicate identity groups: 1
- Source kind counts: `{"arxiv_abs_url": 15, "arxiv_pdf_url": 4, "company_blog_url": 1, "nature_article_url": 1}`

## PDF Acquisition Counts
- Status counts: `{"acquired_existing_pdf": 4, "not_acquired": 15, "not_applicable": 2}`
- Non-acquired/not-applicable reasons: `{"arxiv_abs_no_local_pdf_artifact": 15, "not_applicable_non_arxiv_pdf_source": 2}`
- Existing PDF artifacts: 4
- Candidate refs: 19

## Safety Flags
- All fail-closed flags false: `{"chunk_content_embedded": false, "dspy_attempted": false, "graph_write_attempted": false, "html_source_embedded": false, "kg_readiness_claimed": false, "ladybugdb_written": false, "minimax_attempted": false, "parser_readiness_claimed": false, "production_import_attempted": false, "production_persistence_attempted": false, "raw_article_text_embedded": false, "raw_pdf_bytes_embedded": false, "rlm_attempted": false}`
- Unsafe claim counts: `{"chunk_content_embedded": 0, "dspy_attempted": 0, "graph_write_attempted": 0, "html_source_embedded": 0, "kg_readiness_claimed": 0, "ladybugdb_written": 0, "minimax_attempted": 0, "parser_readiness_claimed": 0, "production_import_attempted": 0, "production_persistence_attempted": 0, "raw_article_text_embedded": 0, "raw_pdf_bytes_embedded": 0, "rlm_attempted": 0}`

## Failure Modes
- selection JSON: missing, malformed, stale-count, duplicate, or required-field errors raise PdfDiagnosticInputError before output write
- source acquisition JSONL: missing, malformed, duplicate, non-terminal, missing-linkage, source-kind drift, or identity drift raise stable input errors
- source metadata events/summary: missing, malformed, stale count, missing linkage, source-kind drift, or identity drift raise stable input errors
- captured PDF artifact filesystem: unsafe paths, missing files, checksum mismatches, non-PDF artifacts, and malformed signatures become typed per-ref non-acquired diagnostics

## Load Profile
- Expected refs: 21; 10x refs: 210
- First saturating resource: sequential filesystem reads and streaming checksum/signature probes for existing PDF artifacts
- Protection: no network calls, one-pass chunked SHA-256 hashing, five-byte signature probe, deterministic per-ref iteration, no parser/graph/KG/model production paths

## Negative Tests
- `tests/test_m028_pdf_acquisition_diagnostics.py::test_malformed_existing_pdf_signature_becomes_typed_diagnostic`
- `tests/test_m028_pdf_acquisition_diagnostics.py::test_missing_acquisition_linkage_is_stable_input_error`
- `tests/test_m028_pdf_acquisition_diagnostics.py::test_checksum_mismatch_records_typed_non_acquired_reason`
- `tests/test_m028_pdf_acquisition_diagnostics.py::test_real_corpus_regeneration_contract`

## Observability Impact
- Emits per-ref PDF candidate classification, terminal typed acquisition reason, checksum/signature status, duplicate identity membership, diagnostics, and fail-closed aggregate counters.
