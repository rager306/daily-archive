# M031 Catalog-Backed Source Acquisition Replay Report

This report is metadata-only and local-only. It does not embed article text, HTML snippets, PDF bytes, or base64 payloads.

- Milestone: `M031-vwpd8e`
- Slice: `S02`
- Selection: `m031-catalog-backed-replay-v1`
- Status: `completed_with_diagnostics`
- Captured: 3
- Blocked: 4
- Failed: 0
- Network fetch attempted count: 0
- Graph/import/LadybugDB writes: false

## Failure Modes

- Missing or null local source paths become `missing_local_source_path` blocked rows.
- Absent local artifacts become `local_source_missing` blocked rows.
- Empty local artifacts become `empty_local_source` failed rows.
- Malformed JSON or unsafe input paths fail the CLI with typed diagnostics.
- Network dependencies are deliberately absent: no fetch code path exists.

## Load Profile

The replay is bounded by the selected variants and copies one file per materialized local source. At 10x this four-ref corpus, disk I/O and metadata report size saturate before CPU or memory; recursive copying, network fetches, and catalog tree scans are not used.

## Negative Tests

Covered in `tests/test_m031_catalog_backed_acquisition_loader.py`: null paths, external PDF metadata-only blockers, typed catalog blocker rows, unsafe `../` source paths, absent artifacts, empty artifacts, and redaction of summary/report text.

## Role Counts

- `arxiv_abs_page`: captured=1 blocked=1 failed=0
- `arxiv_abs_url`: captured=0 blocked=1 failed=0
- `arxiv_html`: captured=1 blocked=0 failed=0
- `arxiv_pdf`: captured=1 blocked=1 failed=0
- `external_pdf`: captured=0 blocked=1 failed=0

## Identity Counts

- `arxiv:2507.19457`: captured=3 blocked=0 failed=0
- `arxiv:2605.26099`: captured=0 blocked=1 failed=0
- `arxiv:2605.29548`: captured=0 blocked=2 failed=0
- `stanford:cs224n:gradient-notes`: captured=0 blocked=1 failed=0

## Results

- `arxiv:2507.19457` `arxiv_html`: captured (captured_local_source_artifact) -> `arxiv/cs-cl/2507.19457/source/article.html`
- `arxiv:2507.19457` `arxiv_pdf`: captured (captured_local_source_artifact) -> `arxiv/cs-cl/2507.19457/source/original.pdf`
- `arxiv:2507.19457` `arxiv_abs_page`: captured (captured_local_source_artifact) -> `arxiv/cs-cl/2507.19457/source/abs.html`
- `stanford:cs224n:gradient-notes` `external_pdf`: blocked (missing_local_source_path) -> `<none>`
- `arxiv:2605.29548` `arxiv_abs_page`: blocked (missing_local_source_path) -> `<none>`
- `arxiv:2605.29548` `arxiv_pdf`: blocked (missing_local_source_path) -> `<none>`
- `arxiv:2605.26099` `arxiv_abs_url`: blocked (catalog_placeholder_pruned_no_article_record) -> `<none>`
