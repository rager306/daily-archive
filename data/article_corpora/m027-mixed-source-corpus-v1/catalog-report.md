# M027 Mixed Source Catalog Validation Report

## Overview
- Selection: `m027-mixed-source-corpus-v1`
- Articles selected: 6
- Source variants captured: 11/12
- S02 blockers: 6
- Ready for S02 parser/chunking baseline: False
- Network replay guarantee: validation uses catalog/index/loader artifacts only and does not fetch.

## Index Evidence
- Lookup surface: `index.json`
- Full tree scan attempted during validation: False
- Rebuild report: `/root/daily-archive/data/article_catalog/index-rebuild-report.json` (present=True)
- Existing index matches rebuild: False
- Idempotent rebuild evidence: True
- Network fetch attempted: False

## Article Readiness
| Article | Provider | Topic | Primary | Captured Variants | PDF Fallback | Blocked Before S02 |
|---|---:|---:|---|---:|---:|---:|
| arxiv/mixed-source/2605.20897 | arxiv | mixed-source | arxiv_abs_page / html_metadata / not_loaded_metadata_only | 2/2 | True | True |
| arxiv/mixed-source/2605.21401 | arxiv | mixed-source | arxiv_abs_page / html_metadata / not_loaded_metadata_only | 2/2 | True | True |
| nature/mixed-source/s44387-025-00019-5 | nature | mixed-source | nature_html / html_metadata / not_loaded_metadata_only | 1/2 | False | True |
| arxiv/mixed-source/2605.25522 | arxiv | mixed-source | arxiv_abs_page / html_metadata / not_loaded_metadata_only | 2/2 | True | True |
| arxiv/mixed-source/2603.04448 | arxiv | mixed-source | arxiv_abs_page / html_metadata / not_loaded_metadata_only | 2/2 | True | True |
| arxiv/mixed-source/2604.18478 | arxiv | mixed-source | arxiv_abs_page / html_metadata / not_loaded_metadata_only | 2/2 | True | True |

## Source Variant Diagnostics
| Article | Role | Format | Primary | Capability | Capture | Checksum | Loader Outcome | Fallback Reason |
|---|---|---|---:|---|---|---|---|---|
| arxiv/mixed-source/2605.20897 | arxiv_abs_page | html_metadata | True | metadata | captured | e1bcfe65b0b7 | not_loaded_metadata_only |  |
| arxiv/mixed-source/2605.20897 | arxiv_pdf | pdf | False | content | captured | b0265c4651cb | not_loaded | future_pdf_to_markdown_conversion_after_acquisition |
| arxiv/mixed-source/2605.21401 | arxiv_abs_page | html_metadata | True | metadata | captured | d09b8a50de56 | not_loaded_metadata_only |  |
| arxiv/mixed-source/2605.21401 | arxiv_pdf | pdf | False | content | captured | 6f6aa8f43aa6 | not_loaded | future_pdf_to_markdown_conversion_after_acquisition |
| nature/mixed-source/s44387-025-00019-5 | nature_html | html_metadata | True | metadata | captured | d4189dd89772 | not_loaded_metadata_only |  |
| nature/mixed-source/s44387-025-00019-5 | citation_metadata | metadata | False | metadata | not_captured |  | not_loaded_metadata_only |  |
| arxiv/mixed-source/2605.25522 | arxiv_abs_page | html_metadata | True | metadata | captured | b3f31f94699e | not_loaded_metadata_only |  |
| arxiv/mixed-source/2605.25522 | arxiv_pdf | pdf | False | content | captured | 54214575fc87 | not_loaded | future_pdf_to_markdown_conversion_after_acquisition |
| arxiv/mixed-source/2603.04448 | arxiv_abs_page | html_metadata | True | metadata | captured | 16fceff1b0f1 | not_loaded_metadata_only |  |
| arxiv/mixed-source/2603.04448 | arxiv_pdf | pdf | False | content | captured | 65c081a8134c | not_loaded | future_pdf_to_markdown_conversion_after_acquisition |
| arxiv/mixed-source/2604.18478 | arxiv_abs_page | html_metadata | True | metadata | captured | ae2da91a3bc6 | not_loaded_metadata_only |  |
| arxiv/mixed-source/2604.18478 | arxiv_pdf | pdf | False | content | captured | ddad8666fd63 | not_loaded | future_pdf_to_markdown_conversion_after_acquisition |

## Failure Modes
- Filesystem dependency: missing catalog, index, selection, article records, loader events, or captured source files bubble as validator errors with non-zero exit; artifact writes are atomic temp-file-plus-rename writes.
- Malformed JSON dependency: `load_json` rejects malformed or non-object JSON and the CLI exits non-zero before writing readiness success claims.
- Index drift dependency: `--check-index-idempotent` rebuilds the projection in memory and fails if the existing index no longer matches an idempotent rebuild report.
- Network dependency: S01 report generation has no network mode; tests and pipeline validation report `network_fetch_attempted=false` instead of refreshing implicitly.
- Subprocess dependency: callers observe standard CLI exit codes; failed validation prints concrete diagnostics to stderr.

## Load Profile
- Expected load is six selected articles and their local variants; 10x load first saturates local filesystem reads and optional checksum hashing, not network or database resources.
- Protection: normal validation resolves articles through `index.json`, full tree traversal is confined to explicit rebuild/idempotency checks, and output writes are bounded JSON/JSONL/Markdown files.
- No pool sizing or rate limiting is needed because report generation has no async, network, API, or database runtime dimension.

## Negative Tests
- `tests/test_m025_article_catalog_verifier.py::test_m025_article_catalog_verifier_rejects_index_title_drift` covers index/article title drift.
- `tests/test_m025_article_catalog_verifier.py::test_m025_article_catalog_verifier_rejects_selection_not_in_index` covers invalid selection references.
- `tests/test_m025_article_catalog_verifier.py::test_m025_article_catalog_rebuild_rejects_duplicate_lookup_key` covers duplicate lookup/index rebuild failures.
- `tests/test_m025_article_catalog_verifier.py::test_m025_article_catalog_verifier_rejects_unsafe_index_traversal` covers unsafe catalog-relative paths.
- `tests/test_m025_article_catalog_verifier.py::test_m025_article_catalog_verifier_writes_catalog_readiness_outputs` covers final summary/report/diagnostics generation and planned S02 blocker reporting.

## Observability Impact
- `catalog-summary.json` records article, variant, loader, rebuild, network, blocker, and safety counts for S02 handoff.
- `catalog-diagnostics.jsonl` records machine-readable index, article, and source-variant readiness rows without raw payloads or vectors.
- `catalog-report.md` provides a human-readable handoff stating primary lightweight variants, preserved PDF fallbacks, idempotency evidence, and S02 blockers.

## M027 Local-Only Handoff

- Milestone: `M027-aakeky`
- Slice: `S01`
- Selection: `m027-mixed-source-corpus-v1`
- Validate-only network_fetch_attempted=false; no network acquisition or refresh is performed.
- Fail-closed safety flags keep graph import, trusted fact promotion, production import, and LadybugDB writes disabled.
- Out of scope: captured sources, conversion, parser readiness, chunks, production imports, trusted facts, and LadybugDB writes.

## Seed URL Mapping
| Seed URL | Article Ref | Title |
|---|---|---|
| https://arxiv.org/pdf/2605.20897 | `arxiv/mixed-source/2605.20897` | Creating Robust and Fair Graph Structures for Connectivity and Clustering |
| https://arxiv.org/abs/2605.21401 | `arxiv/mixed-source/2605.21401` | Open-source LLMs administer maximum electric shocks in a Milgram-like obedience experiment |
| https://www.nature.com/articles/s44387-025-00019-5 | `nature/mixed-source/s44387-025-00019-5` | Exploring the role of large language models in the scientific method: from hypothesis to discovery |
| https://arxiv.org/abs/2605.25522 | `arxiv/mixed-source/2605.25522` | Co-Designing Graph-based Approximate Nearest Neighbor Search at Billion Scale for Processing-in-Memory |
| https://arxiv.org/abs/2603.04448 | `arxiv/mixed-source/2603.04448` | SkillNet: Create, Evaluate, and Connect AI Skills |
| https://arxiv.org/abs/2604.18478 | `arxiv/mixed-source/2604.18478` | WorldDB: A Vector Graph-of-Worlds Memory Engine with Ontology-Aware Write-Time Reconciliation |

## Provenance
- Command: `uv run python scripts/verify_m027_mixed_source_catalog.py`
- CWD: `/root/daily-archive`
- Git commit: `5bc99c5aad8875a77f662b9d39bf61b833c20a7b`
- Exit code: 0
- Output hashes are recorded in `catalog-summary.json` for non-self-referential outputs.
