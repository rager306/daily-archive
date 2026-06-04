# M029 Unified Corpus Readiness Report

- Decision: `partial_preprocessing_ready`
- Headline: 11 of 18 articles are ready for local replay review; 7 remain partial because they have zero chunks.
- Article count: 18
- Ready count: 11
- Partial count: 7
- Blocked count: 0
- Runtime evidence count: 11
- Runtime chunk count: 4787

## Dedupe and Provenance

one selected article per article_ref/identity_key; provenance_sources preserve earlier milestone subset membership without inflating article_count

### Provenance source counts

- `M025`: 5
- `M027`: 6
- `M028`: 13

## Final Counts and Block Reasons

| Category | Count |
|---|---:|
| Ready | 11 |
| Partial | 7 |
| Blocked | 0 |
| Zero chunk | 7 |

### Block reasons

- `no_parser_ready_converted_text`: 7

## Article Readiness

| Article | Identity | Provenance | Source strategy | Readiness | Evidence | Chunks | Block reason |
|---|---|---|---|---|---:|---:|---|
| arxiv/cs-ai/2512.24601 | arxiv:2512.24601 | M025 | arxiv_html | ready_for_local_replay_review | 1 | 171 |  |
| arxiv/cs-ai/2605.28617v1 | arxiv:2605.28617v1 | M025 | arxiv_html | ready_for_local_replay_review | 1 | 1048 |  |
| arxiv/cs-cv/2605.26525v1 | arxiv:2605.26525v1 | M025 | arxiv_html | ready_for_local_replay_review | 1 | 190 |  |
| arxiv/cs-cl/2507.19457 | arxiv:2507.19457 | M025 | arxiv_html | ready_for_local_replay_review | 1 | 4 |  |
| company_blog/cs-ir/pageindex_zhang2025pageindex | company_blog:pageindex_zhang2025pageindex | M025 | web_article_html | ready_for_local_replay_review | 1 | 59 |  |
| arxiv/mixed-source/2605.20897 | arxiv:2605.20897 | M027,M028 | arxiv_abs_page | ready_for_local_replay_review | 1 | 337 |  |
| arxiv/mixed-source/2605.21401 | arxiv:2605.21401 | M027,M028 | arxiv_abs_page | ready_for_local_replay_review | 1 | 571 |  |
| nature/mixed-source/s44387-025-00019-5 | nature:s44387-025-00019-5 | M027,M028 | nature_html | ready_for_local_replay_review | 1 | 190 |  |
| arxiv/mixed-source/2605.25522 | arxiv:2605.25522 | M027,M028 | arxiv_abs_page | ready_for_local_replay_review | 1 | 1248 |  |
| arxiv/mixed-source/2603.04448 | arxiv:2603.04448 | M027,M028 | arxiv_abs_page | ready_for_local_replay_review | 1 | 526 |  |
| arxiv/mixed-source/2604.18478 | arxiv:2604.18478 | M027,M028 | arxiv_abs_page | ready_for_local_replay_review | 1 | 443 |  |
| arxiv:2605.23904 | arxiv:2605.23904 | M028 | arxiv_abs_page | partial_zero_chunk_blocked | 0 | 0 | no_parser_ready_converted_text |
| arxiv:2605.22502 | arxiv:2605.22502 | M028 | arxiv_abs_page | partial_zero_chunk_blocked | 0 | 0 | no_parser_ready_converted_text |
| arxiv:2605.28655 | arxiv:2605.28655 | M028 | arxiv_abs_page | partial_zero_chunk_blocked | 0 | 0 | no_parser_ready_converted_text |
| arxiv:2605.26099 | arxiv:2605.26099 | M028 | arxiv_abs_page | partial_zero_chunk_blocked | 0 | 0 | no_parser_ready_converted_text |
| arxiv:2605.22166 | arxiv:2605.22166 | M028 | arxiv_abs_page | partial_zero_chunk_blocked | 0 | 0 | no_parser_ready_converted_text |
| arxiv:2605.22681 | arxiv:2605.22681 | M028 | arxiv_abs_page | partial_zero_chunk_blocked | 0 | 0 | no_parser_ready_converted_text |
| arxiv:2605.26302 | arxiv:2605.26302 | M028 | arxiv_abs_page | partial_zero_chunk_blocked | 0 | 0 | no_parser_ready_converted_text |

## Boundary Decision

This readiness decision is preprocessing/local-replay only. Graph import, trusted KG promotion, production import, LadybugDB writes, network fetches, and raw payload embedding remain fail-closed and out of scope.

## Safety Flags

- Network fetch attempted: `false`
- Production import attempted: `false`
- LadybugDB written: `false`
- Graph import allowed: `false`
- Trusted KG import allowed: `false`
- Raw text embedded in metadata: `false`
- Raw binary embedded in metadata: `false`
