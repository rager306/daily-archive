# M028 S05 Hermes Digest Projection Smoke

Evidence-only consumer digest projection generated from S04 universal-loader evidence bundles. It keeps source selection, acquisition, parser/chunker semantics, graph import, model behavior, raw payloads, and production writes out of scope.

## Scope
- URL refs: 21
- Normalized identities: 20
- Expanded refs R15-R21 present: `["R15", "R16", "R17", "R18", "R19", "R20", "R21"]`
- Duplicate identity groups: `[{"group_id": "identity:arxiv:2605.20897", "has_url_variants": true, "normalized_identity": "arxiv:2605.20897", "ref_ids": ["R01", "R10"], "url_ref_count": 2, "url_variants": ["pdf_url", "abs_url"]}]`
- Source kind counts: `{"arxiv_abs_url": 15, "arxiv_pdf_url": 4, "company_blog_url": 1, "nature_article_url": 1}`

## Source References
- Loader bundles: `tests/tmp_m028_hermes_digest_projection/test_verifier_rejects_payload_0/universal-loader-evidence-bundles.jsonl` sha256=a5c31711b3a010a76f54b6e1d9bad3f8f35d74e62c7ee15e20bd3b5fbac81e50
- Loader summary: `tests/tmp_m028_hermes_digest_projection/test_verifier_rejects_payload_0/universal-loader-evidence-summary.json` sha256=ba67ba81bf65289b3d78262aca2ec512a96369b9d0bf7f994eef22d3e2ba61d3
- Selection ref: `data/article_corpora/m028-universal-loader-runtime-smoke-v1/selection.json`

## Summary
- Source family counts: `{"arxiv": 19, "company_blog": 1, "nature": 1}`
- Source quality counts: `{"source_metadata_only_non_pdf_source": 2, "source_metadata_only_pdf_not_acquired": 15, "source_metadata_with_verified_pdf_artifact": 4}`
- PDF status counts: `{"acquired_existing_pdf": 4, "not_acquired": 15, "not_applicable": 2}`
- Diagnostic counts: `{"upstream_metadata_optional_metadata_missing": 44, "upstream_pdf_arxiv_abs_no_local_pdf_artifact": 15, "upstream_pdf_not_applicable_non_arxiv_pdf_source": 2}`

## Warnings
- Warning diagnostics by item are retained in `items[*].warnings`; aggregate diagnostic counts are `{"upstream_metadata_optional_metadata_missing": 44, "upstream_pdf_arxiv_abs_no_local_pdf_artifact": 15, "upstream_pdf_not_applicable_non_arxiv_pdf_source": 2}`.

## Skipped Diagnostics
- Optional bibliographic fields absent from S04 loader evidence remain null with `metadata_value_not_in_loader_evidence_bundle` diagnostics.
- Parser, chunker, model, graph, KG import, and production writes are skipped by design.

## Safety
- Redaction flags: `{"chunk_payload_embedded": false, "graph_or_kg_claims_embedded": false, "html_source_embedded": false, "local_absolute_paths_embedded": false, "model_output_embedded": false, "raw_article_text_embedded": false, "raw_pdf_bytes_embedded": false, "source_payload_embedded": false}`
- Unsafe counters: `{"binary_payload_embedded": 0, "chunk_content_embedded": 0, "chunk_payload_embedded": 0, "chunker_attempted": 0, "dspy_attempted": 0, "graph_ready_claimed": 0, "graph_write_attempted": 0, "hermes_digest_count": 0, "hermes_digest_generated": 0, "html_source_embedded": 0, "import_eligible_count": 0, "kg_readiness_claimed": 0, "ladybugdb_written": 0, "minimax_attempted": 0, "model_output_embedded": 0, "parser_attempted": 0, "parser_readiness_claimed": 0, "production_import_attempted": 0, "production_persistence_attempted": 0, "promoted_to_fact_count": 0, "raw_article_text_embedded": 0, "raw_pdf_bytes_embedded": 0, "rlm_attempted": 0, "source_payload_embedded": 0}`

## Failure Modes
- S04 universal-loader evidence bundle JSONL: missing, malformed JSONL, non-object rows, scope drift, unsafe payload markers, absolute/escaping artifact paths, or unsafe bundle flags raise HermesDigestProjectionInputError before output writes
- S04 universal-loader evidence summary JSON: missing, malformed JSON, stale aggregate counts, nonzero unsafe counters, or missing selection/source fingerprints raise or localize stable diagnostics before projection writes
- filesystem outputs: output directory creation or file write failures bubble as filesystem exceptions; no network, subprocess, parser, model, graph, or production dependency exists

## Load Profile
- Expected refs: 21; 10x refs: 210
- First saturating resource: linear JSON/JSONL parsing and deterministic serialization of compact per-ref metadata-only digest items
- Protection: single-pass in-memory smoke-corpus processing, chunked SHA-256 for two input files, no source/PDF body reads, and no live network/parser/model/graph calls

## Negative Tests
- `tests/test_m028_hermes_digest_projection.py::test_rejects_scope_drift_before_projection_write`
- `tests/test_m028_hermes_digest_projection.py::test_rejects_payload_markers_and_absolute_paths`
- `tests/test_m028_hermes_digest_projection.py::test_rejects_nonzero_unsafe_counter`

## Observability Impact
- Emits digest-level scope counters, input fingerprints, source refs, redaction flags, unsafe counters, per-ref quality/warning/skipped diagnostics, and stable validation codes/JSON paths for malformed inputs and safety drift.
