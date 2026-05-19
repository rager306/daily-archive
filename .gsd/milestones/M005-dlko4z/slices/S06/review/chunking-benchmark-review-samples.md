# S06 Chunking Benchmark Review Samples

## Review boundary

- These samples are redacted benchmark diagnostics only.
- They contain no raw paper text, chunk text, embeddings, vectors, or production write state.
- They do not authorize trusted KG import.

## Run summary

- Method count: 3
- Methods: baseline_pageindex_semanticchunk, simple_section_window_estimate, structure_aware_control
- Total compared chunks/candidates: 2471
- Total import-eligible chunks: 0
- Recommendation status: `review_required`

## Method comparison

| Method | Chunks | Import eligible | Refused | Source spans | Annotation coverage | Asset linkage | Key caveats |
|---|---:|---:|---:|---:|---:|---:|---|
| baseline_pageindex_semanticchunk | 345 | 0 | 345 | 0.000 | 0.000 | 0.000 | baseline_retrieval_only_not_import_ready, no_annotation_or_asset_linkage |
| structure_aware_control | 1831 | 0 | 1831 | 1.000 | 1.000 | 0.155 | control_chunker_not_final_algorithm, all_chunks_remain_import_blocked |
| simple_section_window_estimate | 295 | 0 | 295 | 1.000 | 0.000 | 0.959 | estimated_candidate_not_real_chunker, uses_s05_asset_links_without_raw_text_serialization, chonkie_llamaindex_langchain_not_executed |

## Route distribution deltas

### baseline_pageindex_semanticchunk
- `retrieval_only`: 345

### structure_aware_control
- `citation_graph`: 11
- `claim_extraction`: 245
- `metadata_graph`: 2
- `method_extraction`: 136
- `retrieval_only`: 1399
- `table_extraction`: 38

### simple_section_window_estimate
- `citation_graph`: 11
- `metadata_graph`: 2
- `retrieval_only`: 244
- `table_extraction`: 38

## Refusal and missing-source caveats

### baseline_pageindex_semanticchunk
- refusal `baseline_retrieval_only_not_import_ready`: 345

### structure_aware_control
- refusal `administrative_metadata_requires_review`: 2
- refusal `citation_route_requires_review`: 11
- refusal `claim_route_requires_review`: 245
- refusal `equation_route_not_import_ready`: 146
- refusal `figure_route_not_import_ready`: 86
- refusal `method_route_requires_review`: 136
- refusal `retrieval_only_not_import_ready`: 1167
- refusal `table_route_requires_review`: 38
- missing source `missing_original_pdf`: 8

### simple_section_window_estimate
- refusal `estimated_candidate_requires_review`: 295
- missing source `missing_original_pdf`: 8

## Review questions

1. Are the compared methods semantically meaningful enough for S06, or still count-only?
2. Does any method have evidence strong enough to unblock S07 isolated import rehearsal?
3. Do missing PDFs materially affect benchmark conclusions?
4. Are real external chunking libraries still necessary before S07?

## Preliminary recommendation

No method is currently import-approved. The benchmark supports review and comparison only: all methods have `import_eligible_chunk_count=0`, and the run recommendation remains `review_required`.
