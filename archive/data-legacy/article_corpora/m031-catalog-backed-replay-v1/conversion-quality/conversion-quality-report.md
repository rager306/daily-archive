# M031 Parser Conversion Replay Report

This report is metadata-only. It does not embed source HTML, PDF bytes, converted text snippets, base64 payloads, network fetch results, graph facts, or LadybugDB readiness claims.

- Schema: `m031-parser-conversion-replay.v1`
- Status: `completed_with_diagnostics`
- Row count: 7
- Parser-ready converted rows: 1
- Counts: `{'blocked': 3, 'converted': 1, 'low_quality': 1, 'metadata_only': 2}`
- Network fetch attempted: `False`
- arxiv2md invoked: `False`
- md_converter invoked: `False`
- Graph/import/LadybugDB writes: `False`

## Failure Modes

Malformed JSON/setup exits with a typed CLI diagnostic. Row-level missing files, unsafe paths, unsupported media types, absent PyMuPDF, loader blockers, and extraction failures become non-parser-ready diagnostics rather than silent success.

## Load Profile

The replay is bounded by S02 loader result rows, first 8 PDF pages, and 80000 extracted characters per source. At 10x the expected four-ref corpus, local disk reads/PDF parsing saturate first; there is no network, subprocess, graph import, or cache path.

## Negative Tests

Covered by `tests/test_m031_parser_conversion_replay.py`: unsafe `../` paths, missing local source, fallback/short HTML, metadata-only abs page, typed loader blockers, absent PyMuPDF, malformed JSON, metadata/report redaction, and fail-closed graph/import flags.

## Results

- `arxiv:2507.19457` `arxiv_html`: low_quality (converted_text_low_quality) safe_path=`arxiv/cs-cl/2507.19457/source/article.html` parser_ready=False
- `arxiv:2507.19457` `arxiv_pdf`: converted (parser_ready_converted_text) safe_path=`arxiv/cs-cl/2507.19457/source/original.pdf` parser_ready=True
- `arxiv:2507.19457` `arxiv_abs_page`: metadata_only (metadata_only_refused) safe_path=`arxiv/cs-cl/2507.19457/source/abs.html` parser_ready=False
- `stanford:cs224n:gradient-notes` `external_pdf`: blocked (missing_local_source_path) safe_path=`<none>` parser_ready=False
- `arxiv:2605.29548` `arxiv_abs_page`: metadata_only (metadata_only_refused) safe_path=`<none>` parser_ready=False
- `arxiv:2605.29548` `arxiv_pdf`: blocked (missing_local_source_path) safe_path=`<none>` parser_ready=False
- `arxiv:2605.26099` `arxiv_abs_url`: blocked (catalog_placeholder_pruned_no_article_record) safe_path=`<none>` parser_ready=False
