# M028 Mixed Source Acquisition Report

Created: 2026-06-02T10:41:54.903171+00:00

## Scope

Refreshed the accepted expanded M028 smoke corpus to 21 URL refs / 20 normalized identities. This report is metadata-only: it points to captured source artifacts or typed terminal blockers but does not embed raw article text, HTML bodies, or PDF bytes.

## Counts

- URL refs: 21
- Unique normalized identities: 20
- Captured: 21
- Blocked: 0
- Failed: 0
- Source kinds: `{"arxiv_abs_url": 15, "arxiv_pdf_url": 4, "company_blog_url": 1, "nature_article_url": 1}`

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
| R15 | `arxiv:2605.23904` | `arxiv_abs_url` | `captured` | 200 | 49171 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R15-arxiv-2605_23904.html` |
| R16 | `arxiv:2605.22502` | `arxiv_abs_url` | `captured` | 200 | 46423 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R16-arxiv-2605_22502.html` |
| R17 | `arxiv:2605.28655` | `arxiv_abs_url` | `captured` | 200 | 47805 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R17-arxiv-2605_28655.html` |
| R18 | `arxiv:2605.26099` | `arxiv_abs_url` | `captured` | 200 | 45820 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R18-arxiv-2605_26099.html` |
| R19 | `arxiv:2605.22166` | `arxiv_abs_url` | `captured` | 200 | 47010 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R19-arxiv-2605_22166.html` |
| R20 | `arxiv:2605.22681` | `arxiv_abs_url` | `captured` | 200 | 48395 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R20-arxiv-2605_22681.html` |
| R21 | `arxiv:2605.26302` | `arxiv_abs_url` | `captured` | 200 | 48435 | `data/article_corpora/m028-universal-loader-runtime-smoke-v1/sources/R21-arxiv-2605_26302.html` |

## Failure Modes

- External dependencies: arXiv/Nature/NVIDIA HTTPS endpoints, local filesystem writes under `sources/`, JSON/JSONL serialization, and checksum reads.
- HTTP errors produce terminal `blocked` events with `failure_code=http_error`; URL/connection loss produces terminal `blocked` events with `failure_code=connection_error`; timeouts and unexpected fetch exceptions produce terminal `failed` events with typed codes.
- Existing artifacts are not re-fetched; they are treated as local filesystem dependencies and revalidated by size and SHA-256 before summary publication.

## Load Profile

- Expected load is 21 refs; 10x load would saturate remote endpoint politeness and wall-clock latency before local checksum or JSON serialization.
- Protection: serial fetches, `2` attempts per missing ref, `25` second request timeout, and preservation of already captured artifacts to avoid unnecessary network calls.

## Negative Tests

- Verification asserts every selected ref has exactly one terminal event, captured artifacts exist, byte counts and SHA-256 match, and all graph/write/readiness flags remain false.
- Additional diagnostic verification covers missing selected refs, missing new refs R15-R21, duplicate identity group preservation, source-kind count drift, stale 14-ref scope, and non-empty report generation.

## Safety

- `raw_article_text_embedded_in_summary`: `false`
- `binary_payload_embedded_in_summary`: `false`
- `graph_write_attempted`: `false`
- `production_persistence_attempted`: `false`
- `kg_readiness_claimed`: `false`
- `parser_readiness_claimed`: `false`
- `dspy_rlm_minimax_attempted`: `false`

## Next use

Later M028 metadata-adapter tasks should consume `selection.json`, `source-acquisition-events.jsonl`, and `source-acquisition-summary.json`. Captured files are source artifacts only; this report does not claim parser readiness, graph readiness, or production persistence.
