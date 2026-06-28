# M027 Source Acquisition Report

This report is metadata-only. It does not embed article text, abstracts, HTML snippets, PDF text, binary bytes, or base64 payloads.

- Milestone: `M027-aakeky`
- Slice: `S02`
- Selection: `m027-mixed-source-corpus-v1`
- Status: `captured`
- Exit-code style status: `0`
- Command: `scripts/capture_m027_mixed_source_sources.py --catalog data/article_catalog/catalog.json --catalog-root data/article_catalog --index data/article_catalog/index.json --selection data/article_corpora/m027-mixed-source-corpus-v1/selection.json --output-dir data/article_corpora/m027-mixed-source-corpus-v1`
- CWD: `/root/daily-archive`
- Git commit: `1d90ce6`
- Captured: 11
- Blocked: 0
- Failed: 0
- Capture phase network allowed: true
- Replay phase network allowed: false
- Graph import allowed: false
- Production LadybugDB write allowed: false
- Trusted KG import allowed: false
- Production import attempted: false
- LadybugDB written: false

## Inputs

- `catalog`: `data/article_catalog/catalog.json` sha256=`a11a9abe550426fd4893d52912ae9270a441ff2099ad726a95fe05941e580099`
- `index`: `data/article_catalog/index.json` sha256=`a3a56fefc8a554e8abfb15eb26171cbe1e45f86c4db52ab83e82bb40e8cd7f37`
- `selection`: `data/article_corpora/m027-mixed-source-corpus-v1/selection.json` sha256=`fa8e475c9264b18b992f323227df9312619a65ef11348d74d17e44149214d3cf`
- `article:article:3`: `/root/daily-archive/data/article_catalog/article_catalog/arxiv/mixed-source/2605.20897/article.json` sha256=`d37eae0b1ef5dbc9cfe6e6866b328ad3110107b732c4e02ba31b275ffdb2969d`
- `article:article:4`: `/root/daily-archive/data/article_catalog/article_catalog/arxiv/mixed-source/2605.21401/article.json` sha256=`a7b27d9c446393e12abdd2838948b6d83aa04a601e69ab70810d5c0c860b0981`
- `article:article:5`: `/root/daily-archive/data/article_catalog/article_catalog/nature/mixed-source/s44387-025-00019-5/article.json` sha256=`c32fda54e5c7f73d064d0ed8ebc8a8a364ee7d0174aa68ea549dea2a79495db7`
- `article:article:6`: `/root/daily-archive/data/article_catalog/article_catalog/arxiv/mixed-source/2605.25522/article.json` sha256=`5c386b37ce87c9ad42c745a9e094b51ede3861a802e5a7956f6364c85cb20c6c`
- `article:article:7`: `/root/daily-archive/data/article_catalog/article_catalog/arxiv/mixed-source/2603.04448/article.json` sha256=`eae5597362d5644d19baf0ecab057aa5aa41b5aa4ea2c7999e05c233974b8b22`
- `article:article:8`: `/root/daily-archive/data/article_catalog/article_catalog/arxiv/mixed-source/2604.18478/article.json` sha256=`8bef60ab90ef900fd315390f40e4b1231a84f95bc9e9aa5924253bc4628c9571`

## Outputs

- `summary`: `/root/daily-archive/data/article_corpora/m027-mixed-source-corpus-v1/source-acquisition-summary.json` sha256=`7b523cf97e99f387050119bd0179c222d31c58dd95bffcd120b8b9439c0618b1`
- `diagnostics`: `/root/daily-archive/data/article_corpora/m027-mixed-source-corpus-v1/source-acquisition-diagnostics.jsonl` sha256=`dfb679967b3610acafe34f9e8c9c577c389074254e263e28309240d905f4bf26`
- `report`: `/root/daily-archive/data/article_corpora/m027-mixed-source-corpus-v1/source-acquisition-report.md` sha256=`e99d791acec7ab6ce097ac387b03ca6d9c51e53258836831932c01155d72724c`

## Article Counts

- `arxiv/mixed-source/2605.20897`: selected=2 captured=2 blocked=0 failed=0
- `arxiv/mixed-source/2605.21401`: selected=2 captured=2 blocked=0 failed=0
- `nature/mixed-source/s44387-025-00019-5`: selected=1 captured=1 blocked=0 failed=0
- `arxiv/mixed-source/2605.25522`: selected=2 captured=2 blocked=0 failed=0
- `arxiv/mixed-source/2603.04448`: selected=2 captured=2 blocked=0 failed=0
- `arxiv/mixed-source/2604.18478`: selected=2 captured=2 blocked=0 failed=0

## Variants

- `arxiv/mixed-source/2605.20897` `arxiv_abs_page`: captured (captured_source_artifact) -> `source/abs.html`
- `arxiv/mixed-source/2605.20897` `arxiv_pdf`: captured (captured_source_artifact) -> `source/original.pdf`
- `arxiv/mixed-source/2605.21401` `arxiv_abs_page`: captured (captured_source_artifact) -> `source/abs.html`
- `arxiv/mixed-source/2605.21401` `arxiv_pdf`: captured (captured_source_artifact) -> `source/original.pdf`
- `nature/mixed-source/s44387-025-00019-5` `nature_html`: captured (captured_source_artifact) -> `source/article.html`
- `arxiv/mixed-source/2605.25522` `arxiv_abs_page`: captured (captured_source_artifact) -> `source/abs.html`
- `arxiv/mixed-source/2605.25522` `arxiv_pdf`: captured (captured_source_artifact) -> `source/original.pdf`
- `arxiv/mixed-source/2603.04448` `arxiv_abs_page`: captured (captured_source_artifact) -> `source/abs.html`
- `arxiv/mixed-source/2603.04448` `arxiv_pdf`: captured (captured_source_artifact) -> `source/original.pdf`
- `arxiv/mixed-source/2604.18478` `arxiv_abs_page`: captured (captured_source_artifact) -> `source/abs.html`
- `arxiv/mixed-source/2604.18478` `arxiv_pdf`: captured (captured_source_artifact) -> `source/original.pdf`

## Local-Only Replay Verification

This section is metadata-only and does not embed article text, HTML snippets, PDF text, binary bytes, or base64 payloads.
- Verifier schema: `m027-source-acquisition-replay.v1`
- Validate only: `True`
- Network fetch attempted: `False`
- Production import attempted: `False`
- LadybugDB written: `False`
- Trusted KG import allowed: `False`
- Graph import allowed: `False`
- Selected articles verified: 6
- Selected source variants verified: 11
- Exit code: 0
- Error diagnostics: 0
- Command: `['uv', 'run', 'python', 'scripts/verify_m027_source_acquisition_boundary.py']`
- CWD: `/root/daily-archive`
- Git commit: `823b7b79d58b4a13259b71719433f4762193354d`
