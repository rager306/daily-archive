# M029 Unified Loader Runtime Smoke

- Schema: `m029-runtime-smoke.v1`
- Selection: `m029-unified-corpus-v1`
- Article count: 18
- Runtime loaded count: 11
- Zero-chunk count: 7
- Runtime evidence count: 11
- Network fetch attempted: `false`
- Production import attempted: `false`
- LadybugDB written: `false`

## Article Outcomes

| Article | Identity | Source strategy | Outcome | Evidence | Chunks | Diagnostic |
|---|---|---|---:|---:|---:|---|
| arxiv/cs-ai/2512.24601 | arxiv:2512.24601 | arxiv_html | loaded | 1 | 171 | runtime_loader_loaded |
| arxiv/cs-ai/2605.28617v1 | arxiv:2605.28617v1 | arxiv_html | loaded | 1 | 1048 | runtime_loader_loaded |
| arxiv/cs-cv/2605.26525v1 | arxiv:2605.26525v1 | arxiv_html | loaded | 1 | 190 | runtime_loader_loaded |
| arxiv/cs-cl/2507.19457 | arxiv:2507.19457 | arxiv_html | loaded | 1 | 4 | runtime_loader_loaded |
| company_blog/cs-ir/pageindex_zhang2025pageindex | company_blog:pageindex_zhang2025pageindex | web_article_html | loaded | 1 | 59 | runtime_loader_loaded |
| arxiv/mixed-source/2605.20897 | arxiv:2605.20897 | arxiv_abs_page | loaded | 1 | 337 | runtime_loader_loaded |
| arxiv/mixed-source/2605.21401 | arxiv:2605.21401 | arxiv_abs_page | loaded | 1 | 571 | runtime_loader_loaded |
| nature/mixed-source/s44387-025-00019-5 | nature:s44387-025-00019-5 | nature_html | loaded | 1 | 190 | runtime_loader_loaded |
| arxiv/mixed-source/2605.25522 | arxiv:2605.25522 | arxiv_abs_page | loaded | 1 | 1248 | runtime_loader_loaded |
| arxiv/mixed-source/2603.04448 | arxiv:2603.04448 | arxiv_abs_page | loaded | 1 | 526 | runtime_loader_loaded |
| arxiv/mixed-source/2604.18478 | arxiv:2604.18478 | arxiv_abs_page | loaded | 1 | 443 | runtime_loader_loaded |
| arxiv:2605.23904 | arxiv:2605.23904 | arxiv_abs_page | zero_chunk | 0 | 0 | runtime_loader_zero_chunk |
| arxiv:2605.22502 | arxiv:2605.22502 | arxiv_abs_page | zero_chunk | 0 | 0 | runtime_loader_zero_chunk |
| arxiv:2605.28655 | arxiv:2605.28655 | arxiv_abs_page | zero_chunk | 0 | 0 | runtime_loader_zero_chunk |
| arxiv:2605.26099 | arxiv:2605.26099 | arxiv_abs_page | zero_chunk | 0 | 0 | runtime_loader_zero_chunk |
| arxiv:2605.22166 | arxiv:2605.22166 | arxiv_abs_page | zero_chunk | 0 | 0 | runtime_loader_zero_chunk |
| arxiv:2605.22681 | arxiv:2605.22681 | arxiv_abs_page | zero_chunk | 0 | 0 | runtime_loader_zero_chunk |
| arxiv:2605.26302 | arxiv:2605.26302 | arxiv_abs_page | zero_chunk | 0 | 0 | runtime_loader_zero_chunk |

## Fail-Closed Boundaries

Runtime smoke is metadata-only: it does not fetch network sources, does not import graph state, does not write LadybugDB, and does not embed raw converted text in summary/diagnostic/report metadata.
