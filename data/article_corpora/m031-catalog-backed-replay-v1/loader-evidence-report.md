# M031 Catalog-Backed Loader Evidence Replay Report

This report is metadata-only and local-only. It does not embed article text, HTML snippets, PDF bytes, or base64 payloads.

- Milestone: `M031-vwpd8e`
- Slice: `S02`
- Selection: `m031-catalog-backed-replay-v1`
- Status: `completed_with_diagnostics`
- Loader attempted: 3
- Loaded: 2
- Loaded metadata only: 1
- Failed: 0
- Loader blocked: 4
- Network fetch attempted count: 0
- Graph/import/LadybugDB writes: false

## Failure Modes

- Malformed selection or acquisition JSON fails closed with a typed CLI diagnostic.
- Selection/acquisition ID mismatch fails closed before loader calls.
- Non-captured acquisition rows become loader blocker rows and are never passed to the loader.
- Missing, unsafe, hash-mismatched, or size-mismatched captured files become loader blockers with stable diagnostics.
- Loader failures are terminal evidence rows; they do not imply parser, chunk, graph, import, or LadybugDB readiness.
- Network dependencies are deliberately absent: no fetch code path exists.

## Load Profile

The replay is bounded by captured acquisition rows and performs one local loader call per captured artifact. At 10x this four-ref corpus, disk reads and JSONL event volume grow linearly and saturate before CPU; there is no network, subprocess, graph write, or recursive catalog scan path.

## Negative Tests

Covered in `tests/test_m031_catalog_backed_acquisition_loader.py`: blocked acquisition rows not loaded, PDF metadata-only classification, missing captured file, acquisition hash mismatch, raw text redaction, unsafe loader event path, malformed acquisition shape, selection/acquisition mismatch, and true safety flag rejection.

## Role Counts

- `arxiv_abs_page`: loaded=1 loaded_metadata_only=0 failed=0 blocked=1
- `arxiv_abs_url`: loaded=0 loaded_metadata_only=0 failed=0 blocked=1
- `arxiv_html`: loaded=1 loaded_metadata_only=0 failed=0 blocked=0
- `arxiv_pdf`: loaded=0 loaded_metadata_only=1 failed=0 blocked=1
- `external_pdf`: loaded=0 loaded_metadata_only=0 failed=0 blocked=1

## Results

- `arxiv:2507.19457` `arxiv_html`: loaded (loader_loaded) -> `arxiv/cs-cl/2507.19457/source/article.html`; text_present=True
- `arxiv:2507.19457` `arxiv_pdf`: loaded_metadata_only (loader_loaded_metadata_only) -> `arxiv/cs-cl/2507.19457/source/original.pdf`; text_present=False
- `arxiv:2507.19457` `arxiv_abs_page`: loaded (loader_loaded) -> `arxiv/cs-cl/2507.19457/source/abs.html`; text_present=True
- `stanford:cs224n:gradient-notes` `external_pdf`: blocked (acquisition_not_captured) -> `<none>`; text_present=False
- `arxiv:2605.29548` `arxiv_abs_page`: blocked (acquisition_not_captured) -> `<none>`; text_present=False
- `arxiv:2605.29548` `arxiv_pdf`: blocked (acquisition_not_captured) -> `<none>`; text_present=False
- `arxiv:2605.26099` `arxiv_abs_url`: blocked (acquisition_not_captured) -> `<none>`; text_present=False
