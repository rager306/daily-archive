# M027 Source Acquisition Report

This report is metadata-only. It does not embed article text, abstracts, HTML snippets, PDF text, binary bytes, or base64 payloads.

- Milestone: `M027-aakeky`
- Slice: `S02`
- Selection: `m027-mixed-source-corpus-v1`
- Status: `captured`
- Exit-code style status: `0`
- Command: `scripts/capture_m027_mixed_source_sources.py --catalog data/article_catalog/catalog.json --catalog-root data/article_catalog --index data/article_catalog/index.json --selection data/article_corpora/m027-mixed-source-corpus-v1/selection.json --output-dir data/article_corpora/m027-mixed-source-corpus-v1`
- CWD: `/root/daily-archive`
- Git commit: `41018a6`
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
- `article:article:3`: `/root/daily-archive/data/article_catalog/article_catalog/arxiv/mixed-source/2605.20897/article.json` sha256=`48b19e7e8de25bdb7e37dd2f6e447c829255963df3372224998bf20397f9bf7d`
- `article:article:4`: `/root/daily-archive/data/article_catalog/article_catalog/arxiv/mixed-source/2605.21401/article.json` sha256=`6cba0fe06098cd501cf0251fbeb13a9422ea91454300d8c8f357d25306399d1b`
- `article:article:5`: `/root/daily-archive/data/article_catalog/article_catalog/nature/mixed-source/s44387-025-00019-5/article.json` sha256=`ee86dd6ca1036bed276688a42be5f8035864f320ef14207244d2e6ca356e6f1c`
- `article:article:6`: `/root/daily-archive/data/article_catalog/article_catalog/arxiv/mixed-source/2605.25522/article.json` sha256=`f61f1a1aace6cfcc64a7214c0f125615d5e3e3cc30d0bbb86ce75a8bd053b428`
- `article:article:7`: `/root/daily-archive/data/article_catalog/article_catalog/arxiv/mixed-source/2603.04448/article.json` sha256=`06c2dfac7aafa7b30203ee3d2118827d06e6a3119815f831669eec9cad7788ec`
- `article:article:8`: `/root/daily-archive/data/article_catalog/article_catalog/arxiv/mixed-source/2604.18478/article.json` sha256=`5501e912997a2477a683dba53d4696daa40d301354b16c5ac20c9146c9f58c35`

## Outputs

- `summary`: `/root/daily-archive/data/article_corpora/m027-mixed-source-corpus-v1/source-acquisition-summary.json` sha256=`d14dd79945d7e8fa5cd4dda8f3ea52d4bab3cf34ffd7f143171b0a6f0c10789d`
- `diagnostics`: `/root/daily-archive/data/article_corpora/m027-mixed-source-corpus-v1/source-acquisition-diagnostics.jsonl` sha256=`24dfa44d93b885c7eb997a82e4f5ed4ef20dbd6d2a421d5177a4139e1515b25e`
- `report`: `/root/daily-archive/data/article_corpora/m027-mixed-source-corpus-v1/source-acquisition-report.md` sha256=`d9625d75f368378a174780251543aa95fe17bcde223391cd4beda6e04236ad65`

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
