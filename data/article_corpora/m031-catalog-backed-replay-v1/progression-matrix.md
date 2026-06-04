# M031 Progression Matrix

Metadata-only per-ref/module progression through M031. No raw article text, chunk text, PDF bytes, HTML, embeddings, vectors, graph facts, or LadybugDB writes are included.

## Per-Ref / Module Progression Matrix

| Row | Identity | Source Role | Package | Parser Ready | Chunks | Review State | Import Boundary | Refusal Reasons |
|---|---|---|---|---:|---:|---|---|---|
| M031-PROGRESSION-001 | `arxiv:2507.19457` | `arxiv_html` | `arxiv_cs-cl_2507.19457_arxiv_html` | false | 0 | `not_applicable_zero_chunk_refusal` | `refused` | `non_parser_ready_zero_chunk_refusal:converted_text_low_quality` |
| M031-PROGRESSION-002 | `arxiv:2507.19457` | `arxiv_pdf` | `arxiv_cs-cl_2507.19457_arxiv_pdf` | true | 8 | `pending_review` | `refused` | `completed_independent_graph_readiness_review_required` |
| M031-PROGRESSION-003 | `arxiv:2507.19457` | `arxiv_abs_page` | `arxiv_cs-cl_2507.19457_arxiv_abs_page` | false | 0 | `not_applicable_zero_chunk_refusal` | `refused` | `non_parser_ready_zero_chunk_refusal:metadata_only_refused` |
| M031-PROGRESSION-004 | `stanford:cs224n:gradient-notes` | `external_pdf` | `stanford_cs224n_gradient-notes_external_pdf` | false | 0 | `not_applicable_zero_chunk_refusal` | `refused` | `non_parser_ready_zero_chunk_refusal:missing_local_source_path` |
| M031-PROGRESSION-005 | `arxiv:2605.29548` | `arxiv_abs_page` | `arxiv_mixed-source_2605.29548_arxiv_abs_page` | false | 0 | `not_applicable_zero_chunk_refusal` | `refused` | `non_parser_ready_zero_chunk_refusal:metadata_only_refused` |
| M031-PROGRESSION-006 | `arxiv:2605.29548` | `arxiv_pdf` | `arxiv_mixed-source_2605.29548_arxiv_pdf` | false | 0 | `not_applicable_zero_chunk_refusal` | `refused` | `non_parser_ready_zero_chunk_refusal:missing_local_source_path` |
| M031-PROGRESSION-007 | `arxiv:2605.26099` | `arxiv_abs_url` | `arxiv_2605.26099_arxiv_abs_url` | false | 0 | `not_applicable_zero_chunk_refusal` | `refused` | `non_parser_ready_zero_chunk_refusal:catalog_placeholder_pruned_no_article_record` |

## Stage Coverage

- `url_intake`: present for all seven rows with evidence path, JSON path, diagnostic code, and status.
- `article_catalog`: present for all seven rows with evidence path, JSON path, diagnostic code, and status.
- `source_acquisition`: present for all seven rows with evidence path, JSON path, diagnostic code, and status.
- `loader_evidence`: present for all seven rows with evidence path, JSON path, diagnostic code, and status.
- `parser_conversion`: present for all seven rows with evidence path, JSON path, diagnostic code, and status.
- `chunking`: present for all seven rows with evidence path, JSON path, diagnostic code, and status.
- `graph_readiness_review`: present for all seven rows with evidence path, JSON path, diagnostic code, and status.
- `graph_import_boundary`: present for all seven rows with evidence path, JSON path, diagnostic code, and status.

## Fail-Closed Flags

- graph_import_allowed=false
- trusted_kg_import_allowed=false
- production_import_attempted=false
- graph_write_attempted=false
- production_persistence_attempted=false
- ladybugdb_written=false
- raw_text_included=false; chunk_text_included=false; embeddings_included=false; vectors_included=false

## Structural Route Label Notice

`ok_for_graph` and `trusted_graph` route labels are structural states only while independent semantic review is incomplete. They are not graph import approval, trusted KG approval, or LadybugDB write authorization.
