# M031 Import Boundary Rehearsal Report

This is a refusal-only, no-write rehearsal. It is not graph or LadybugDB readiness approval.

## Summary
- rehearsal_id: `m031-s05-refusal-only-import-boundary`
- source_benchmark_id: `m031-catalog-backed-replay-v1`
- candidates: 7
- accepted/import-eligible candidates: 0/0
- rejected candidates: 7

## Fail-Closed Flags
- trusted KG import allowed: false
- graph import allowed: false
- production import attempted: false
- LadybugDB writes: false
- network fetch attempted: false

## Diagnostic Codes
- `M031_IMPORT_BOUNDARY_REFUSED`: 7

## Refusal Counts
- `completed_independent_graph_readiness_review_required`: 1
- `non_parser_ready_zero_chunk_refusal:catalog_placeholder_pruned_no_article_record`: 1
- `non_parser_ready_zero_chunk_refusal:converted_text_low_quality`: 1
- `non_parser_ready_zero_chunk_refusal:metadata_only_refused`: 2
- `non_parser_ready_zero_chunk_refusal:missing_local_source_path`: 2

## Candidate Matrix
| JSON Path | Package | Type | Route | State | Refusal Reasons |
|---|---|---|---|---|---|
| $.candidates[0] | `arxiv_cs-cl_2507.19457_arxiv_html` | `zero_chunk_refusal` | `arxiv_html` | `zero_chunk_refused` | `non_parser_ready_zero_chunk_refusal:converted_text_low_quality` |
| $.candidates[1] | `arxiv_cs-cl_2507.19457_arxiv_pdf` | `graph_readiness_package` | `retrieval_only` | `ok_for_retrieval_only` | `completed_independent_graph_readiness_review_required` |
| $.candidates[2] | `arxiv_cs-cl_2507.19457_arxiv_abs_page` | `zero_chunk_refusal` | `arxiv_abs_page` | `zero_chunk_refused` | `non_parser_ready_zero_chunk_refusal:metadata_only_refused` |
| $.candidates[3] | `stanford_cs224n_gradient-notes_external_pdf` | `zero_chunk_refusal` | `external_pdf` | `zero_chunk_refused` | `non_parser_ready_zero_chunk_refusal:missing_local_source_path` |
| $.candidates[4] | `arxiv_mixed-source_2605.29548_arxiv_abs_page` | `zero_chunk_refusal` | `arxiv_abs_page` | `zero_chunk_refused` | `non_parser_ready_zero_chunk_refusal:metadata_only_refused` |
| $.candidates[5] | `arxiv_mixed-source_2605.29548_arxiv_pdf` | `zero_chunk_refusal` | `arxiv_pdf` | `zero_chunk_refused` | `non_parser_ready_zero_chunk_refusal:missing_local_source_path` |
| $.candidates[6] | `arxiv_2605.26099_arxiv_abs_url` | `zero_chunk_refusal` | `arxiv_abs_url` | `zero_chunk_refused` | `non_parser_ready_zero_chunk_refusal:catalog_placeholder_pruned_no_article_record` |

No raw text, chunk text, PDF bytes, HTML, embeddings, vectors, secrets, model traces, optimizer traces, external fetch state, graph writes, or LadybugDB writes are included.
