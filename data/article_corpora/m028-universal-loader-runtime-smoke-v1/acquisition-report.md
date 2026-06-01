# M028 Mixed Source Acquisition Report

Created: 2026-06-01T11:52:38.473943+00:00

## Scope

Aggregated prior M027 user-supplied refs and current M028 refs into a bounded local source-acquisition smoke corpus. This report is metadata-only: it points to captured source artifacts but does not embed raw article text or PDF bytes.

## Counts

- URL refs: 14
- Unique normalized identities: 13
- Captured: 14
- Blocked: 0
- Source kinds: `{"arxiv_abs_url": 8, "arxiv_pdf_url": 4, "company_blog_url": 1, "nature_article_url": 1}`

## Duplicate identities

- `arxiv:2605.20897`: R01, R10

## Per-ref acquisition

| Ref | Identity | Kind | Status | HTTP | Bytes | Artifact / Failure |
|---|---|---|---:|---:|---:|---|
| R01 | `arxiv:2605.20897` | `arxiv_pdf_url` | `captured` | 200 | 2793397 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R01-arxiv-2605_20897.pdf` |
| R02 | `arxiv:2605.21401` | `arxiv_abs_url` | `captured` | 200 | 46318 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R02-arxiv-2605_21401.html` |
| R03 | `nature:articles_s44387-025-00019-5` | `nature_article_url` | `captured` | 200 | 621950 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R03-nature-articles_s44387-025-00019-5.html` |
| R04 | `arxiv:2605.25522` | `arxiv_abs_url` | `captured` | 200 | 47861 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R04-arxiv-2605_25522.html` |
| R05 | `arxiv:2603.04448` | `arxiv_abs_url` | `captured` | 200 | 53753 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R05-arxiv-2603_04448.html` |
| R06 | `arxiv:2604.18478` | `arxiv_abs_url` | `captured` | 200 | 47587 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R06-arxiv-2604_18478.html` |
| R07 | `company_blog:nvidia:dynosim-simulating-the-pareto-frontier` | `company_blog_url` | `captured` | 200 | 295372 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R07-company_blog-nvidia-dynosim-simulating-the-pareto-frontier.html` |
| R08 | `arxiv:2605.20918` | `arxiv_pdf_url` | `captured` | 200 | 2766842 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R08-arxiv-2605_20918.pdf` |
| R09 | `arxiv:2605.18211` | `arxiv_abs_url` | `captured` | 200 | 46103 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R09-arxiv-2605_18211.html` |
| R10 | `arxiv:2605.20897` | `arxiv_abs_url` | `captured` | 200 | 45399 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R10-arxiv-2605_20897.html` |
| R11 | `arxiv:2511.20639` | `arxiv_abs_url` | `captured` | 200 | 48977 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R11-arxiv-2511_20639.html` |
| R12 | `arxiv:2502.13025` | `arxiv_pdf_url` | `captured` | 200 | 12375076 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R12-arxiv-2502_13025.pdf` |
| R13 | `arxiv:2510.21148` | `arxiv_pdf_url` | `captured` | 200 | 1061469 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R13-arxiv-2510_21148.pdf` |
| R14 | `arxiv:2605.21997` | `arxiv_abs_url` | `captured` | 200 | 46358 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R14-arxiv-2605_21997.html` |

## Safety

- `raw_article_text_embedded_in_summary`: `false`
- `binary_payload_embedded_in_summary`: `false`
- `graph_write_attempted`: `false`
- `production_persistence_attempted`: `false`
- `kg_readiness_claimed`: `false`
- `parser_readiness_claimed`: `false`
- `dspy_rlm_minimax_attempted`: `false`

## Next use

Later M028 slices should consume `selection.json`, `source-acquisition-events.jsonl`, and `source-acquisition-summary.json` to build normalized evidence bundles. Captured files are source artifacts only; this report does not claim parser readiness, graph readiness, or production persistence.
