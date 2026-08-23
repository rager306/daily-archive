# M028 S04 Universal Loader Evidence Bundles

Metadata-only evidence bundles for the accepted mixed-source smoke corpus. This report fuses S02 source metadata and S03 PDF diagnostics without live acquisition, parser/chunker work, graph writes, Hermes digest generation, model calls, or raw payload serialization.

## Scope
- URL refs: 21
- Normalized identities: 20
- Duplicate identity groups: 1
- Source kind counts: `{"arxiv_abs_url": 15, "arxiv_pdf_url": 4, "company_blog_url": 1, "nature_article_url": 1}`
- Source quality counts: `{"source_metadata_only_non_pdf_source": 2, "source_metadata_only_pdf_not_acquired": 15, "source_metadata_with_verified_pdf_artifact": 4}`

## Loader Evidence Outcomes
- Bundle status counts: `{"metadata_only_bundle_ready": 21}`
- PDF status counts: `{"acquired_existing_pdf": 4, "not_acquired": 15, "not_applicable": 2}`
- Diagnostics: `{"upstream_metadata_optional_metadata_missing": 44, "upstream_pdf_arxiv_abs_no_local_pdf_artifact": 15, "upstream_pdf_not_applicable_non_arxiv_pdf_source": 2}`

## Safety Flags
- All fail-closed flags false: `{"binary_payload_embedded": false, "chunk_content_embedded": false, "chunk_payload_embedded": false, "chunker_attempted": false, "dspy_attempted": false, "graph_ready_claimed": false, "graph_write_attempted": false, "hermes_digest_generated": false, "html_source_embedded": false, "kg_readiness_claimed": false, "ladybugdb_written": false, "minimax_attempted": false, "model_output_embedded": false, "parser_attempted": false, "parser_readiness_claimed": false, "production_import_attempted": false, "production_persistence_attempted": false, "raw_article_text_embedded": false, "raw_pdf_bytes_embedded": false, "rlm_attempted": false, "source_payload_embedded": false}`
- Unsafe claim counts: `{"binary_payload_embedded": 0, "chunk_content_embedded": 0, "chunk_payload_embedded": 0, "chunker_attempted": 0, "dspy_attempted": 0, "graph_ready_claimed": 0, "graph_write_attempted": 0, "hermes_digest_count": 0, "hermes_digest_generated": 0, "html_source_embedded": 0, "import_eligible_count": 0, "kg_readiness_claimed": 0, "ladybugdb_written": 0, "minimax_attempted": 0, "model_output_embedded": 0, "parser_attempted": 0, "parser_readiness_claimed": 0, "production_import_attempted": 0, "production_persistence_attempted": 0, "promoted_to_fact_count": 0, "raw_article_text_embedded": 0, "raw_pdf_bytes_embedded": 0, "rlm_attempted": 0, "source_payload_embedded": 0}`

## Failure Modes
- selection JSON: missing, malformed, stale-count, duplicate, or required-field errors raise UniversalLoaderEvidenceInputError before output write
- source acquisition JSONL: missing, malformed, duplicate, missing-linkage, source-kind drift, identity drift, or unsafe artifact paths raise stable input errors or typed per-ref diagnostics
- source metadata events/summary: missing, malformed, stale count, missing linkage, source-kind drift, identity drift, or unsafe safety flags raise stable input errors before output write
- PDF acquisition events/summary: missing, malformed, stale count, missing linkage, source-kind drift, identity drift, or unsafe safety flags raise stable input errors before output write
- filesystem outputs: output directory creation or write failures bubble as filesystem exceptions; no network or subprocess dependency exists

## Load Profile
- Expected refs: 21; 10x refs: 210
- First saturating resource: JSON/JSONL input size and deterministic serialization of per-ref metadata-only bundles
- Protection: single-pass in-memory processing for small smoke corpus, no live network/parser/model/graph calls, chunked SHA-256 only for six input artifact fingerprints, and no source/PDF payload reads

## Negative Tests
- `tests/test_m028_universal_loader_evidence_bundles.py::test_fixture_build_preserves_duplicate_identity_and_fail_closed_flags`
- `tests/test_m028_universal_loader_evidence_bundles.py::test_missing_pdf_event_is_stable_input_error`
- `tests/test_m028_universal_loader_evidence_bundles.py::test_upstream_unsafe_flag_is_stable_input_error`
- `tests/test_m028_universal_loader_evidence_bundles.py::test_real_corpus_build_contract`

## Observability Impact
- Emits per-ref metadata-only bundle records, source/PDF diagnostic rollups, duplicate identity membership, input fingerprints, stable diagnostic codes/JSON paths, and fail-closed aggregate counters for downstream S05 inspection.
