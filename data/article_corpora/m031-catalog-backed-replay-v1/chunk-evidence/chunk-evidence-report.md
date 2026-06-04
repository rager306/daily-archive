# M031 Chunk Evidence Replay Report

This S04 report is metadata-only. It does not embed converted text, source HTML, PDF bytes, chunk text, embeddings, vectors, graph facts, or LadybugDB write claims.

- Schema: `m031-chunk-evidence-replay.v1`
- Status: `completed_with_diagnostics`
- Row count: 7
- Chunked parser-ready rows: 1
- Zero-chunk refusals: 6
- Chunk count: 8
- Package count: 1
- Pending graph-readiness reviews: 1
- Import-eligible chunks: `0`
- Network fetch attempted: `False`
- Graph/import/LadybugDB writes: `False`

## Failure Modes

S03 closeout must be present, current, and passed before success artifacts are written. Malformed JSON, stale S03 row/parser-ready counts, unsafe converted paths, missing converted artifacts, hash/size mismatches, and unsafe graph/import flags raise typed deterministic errors. Non-parser-ready S03 rows are preserved as zero-chunk refusal diagnostics instead of being promoted.

## Load Profile

The replay is bounded to the seven S03 conversion rows and reads converted text only for parser-ready rows. At 10x expected load, local CPU/memory for deterministic Markdown structure parsing of converted text saturates first; no corpus-wide scan, network fetch, subprocess, graph import, or LadybugDB write path is used.

## Negative Tests

Covered by `tests/test_m031_chunk_evidence_replay.py`: converted hash mismatch, converted path outside project root, fallback HTML parser-ready promotion, stale/failing S03 closeout, missing converted artifact, missing graph-readiness package blocker event, deleted review Markdown verifier failure, stale placeholder/completed-verdict rejection, non-parser-ready review corpus refusal, raw payload marker redaction, and zero eligibility/fail-closed graph/import flags.

## Graph-Readiness Review Handoff

- Review corpus: `data/article_corpora/m031-catalog-backed-replay-v1/chunk-evidence/review-corpus.json`
- Review events: `data/article_corpora/m031-catalog-backed-replay-v1/chunk-evidence/independent-review-events.jsonl`
- Review summary: `data/article_corpora/m031-catalog-backed-replay-v1/graph-readiness-review/independent-review-summary.md`
- Independent review completed: `0`
- Automated state is structural only: `True`

## Results

- `arxiv:2507.19457` `arxiv_html`: zero_chunk_refused chunks=0 code=`non_parser_ready_zero_chunk_refusal:converted_text_low_quality` package=`<none>`
- `arxiv:2507.19457` `arxiv_pdf`: chunked chunks=8 code=`parser_ready_chunk_package_created` package=`data/article_corpora/m031-catalog-backed-replay-v1/chunk-evidence/packages/arxiv_cs-cl_2507.19457_arxiv_pdf/structure-aware-package.json`
- `arxiv:2507.19457` `arxiv_abs_page`: zero_chunk_refused chunks=0 code=`non_parser_ready_zero_chunk_refusal:metadata_only_refused` package=`<none>`
- `stanford:cs224n:gradient-notes` `external_pdf`: zero_chunk_refused chunks=0 code=`non_parser_ready_zero_chunk_refusal:missing_local_source_path` package=`<none>`
- `arxiv:2605.29548` `arxiv_abs_page`: zero_chunk_refused chunks=0 code=`non_parser_ready_zero_chunk_refusal:metadata_only_refused` package=`<none>`
- `arxiv:2605.29548` `arxiv_pdf`: zero_chunk_refused chunks=0 code=`non_parser_ready_zero_chunk_refusal:missing_local_source_path` package=`<none>`
- `arxiv:2605.26099` `arxiv_abs_url`: zero_chunk_refused chunks=0 code=`non_parser_ready_zero_chunk_refusal:catalog_placeholder_pruned_no_article_record` package=`<none>`
