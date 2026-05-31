# M025 S01 Catalog Readiness Report

## Overview
- Selection: `m025-rlm-dspy-pageindex-smoke-v1`
- Articles selected: 5
- Source variants captured: 12/12
- S02 blockers: 0
- Ready for S02 parser/chunking baseline: True
- Network replay guarantee: validation uses catalog/index/loader artifacts only and does not fetch.

## Index Evidence
- Lookup surface: `index.json`
- Full tree scan attempted during validation: False
- Rebuild report: `data/article_catalog/index-rebuild-report.json` (present=True)
- Existing index matches rebuild: True
- Idempotent rebuild evidence: True
- Network fetch attempted: False

## Article Readiness
| Article | Provider | Topic | Primary | Captured Variants | PDF Fallback | Blocked Before S02 |
|---|---:|---:|---|---:|---:|---:|
| arxiv/cs-ai/2512.24601 | arxiv | cs-ai | arxiv_html / html / loaded | 3/3 | True | False |
| arxiv/cs-ai/2605.28617v1 | arxiv | cs-ai | arxiv_html / html / loaded | 2/2 | True | False |
| arxiv/cs-cv/2605.26525v1 | arxiv | cs-cv | arxiv_html / html / loaded | 2/2 | True | False |
| arxiv/cs-cl/2507.19457 | arxiv | cs-cl | arxiv_html / html / loaded | 3/3 | True | False |
| company_blog/cs-ir/pageindex_zhang2025pageindex | company_blog | cs-ir | web_article_html / html / loaded | 2/2 | False | False |

## Source Variant Diagnostics
| Article | Role | Format | Primary | Capability | Capture | Checksum | Loader Outcome | Fallback Reason |
|---|---|---|---:|---|---|---|---|---|
| arxiv/cs-ai/2512.24601 | arxiv_html | html | True | content | captured | ae416d7e98fe | loaded |  |
| arxiv/cs-ai/2512.24601 | arxiv_pdf | pdf | False | content | captured | 8567362c2276 | loaded_metadata_only | docling_or_marker_pdf_to_markdown |
| arxiv/cs-ai/2512.24601 | arxiv_abs_page | html | False | metadata | captured | a82c188a4c8e | loaded |  |
| arxiv/cs-ai/2605.28617v1 | arxiv_html | html | True | content | captured | bd87e42814f2 | loaded |  |
| arxiv/cs-ai/2605.28617v1 | arxiv_pdf | pdf | False | content | captured | 3b96a71c83dd | loaded_metadata_only | docling_or_marker_pdf_to_markdown |
| arxiv/cs-cv/2605.26525v1 | arxiv_html | html | True | content | captured | 77728ef5c6a7 | loaded |  |
| arxiv/cs-cv/2605.26525v1 | arxiv_pdf | pdf | False | content | captured | a56724976715 | loaded_metadata_only | docling_or_marker_pdf_to_markdown |
| arxiv/cs-cl/2507.19457 | arxiv_html | html | True | content | captured | ab2bfb571582 | loaded |  |
| arxiv/cs-cl/2507.19457 | arxiv_pdf | pdf | False | content | captured | ab3a5139bac8 | loaded_metadata_only | docling_or_marker_pdf_to_markdown |
| arxiv/cs-cl/2507.19457 | arxiv_abs_page | html | False | metadata | captured | bd8f7beb548b | loaded |  |
| company_blog/cs-ir/pageindex_zhang2025pageindex | web_article_html | html | True | content | captured | 03b1623eeb69 | loaded |  |
| company_blog/cs-ir/pageindex_zhang2025pageindex | bibtex_citation | bibtex | False | metadata | captured | 26af78adb09c | failed |  |

## Failure Modes
- Filesystem dependency: missing catalog, index, selection, article records, loader events, or captured source files bubble as validator errors with non-zero exit; artifact writes are atomic temp-file-plus-rename writes.
- Malformed JSON dependency: `load_json` rejects malformed or non-object JSON and the CLI exits non-zero before writing readiness success claims.
- Index drift dependency: `--check-index-idempotent` rebuilds the projection in memory and fails if the existing index no longer matches an idempotent rebuild report.
- Network dependency: S01 report generation has no network mode; tests and pipeline validation report `network_fetch_attempted=false` instead of refreshing implicitly.
- Subprocess dependency: callers observe standard CLI exit codes; failed validation prints concrete diagnostics to stderr.

## Load Profile
- Expected load is five selected articles and their local variants; 10x load first saturates local filesystem reads and optional checksum hashing, not network or database resources.
- Protection: normal validation resolves articles through `index.json`, full tree traversal is confined to explicit rebuild/idempotency checks, and output writes are bounded JSON/JSONL/Markdown files.
- No pool sizing or rate limiting is needed because report generation has no async, network, API, or database runtime dimension.

## Negative Tests
- `tests/test_m025_article_catalog_verifier.py::test_m025_article_catalog_verifier_rejects_index_title_drift` covers index/article title drift.
- `tests/test_m025_article_catalog_verifier.py::test_m025_article_catalog_verifier_rejects_selection_not_in_index` covers invalid selection references.
- `tests/test_m025_article_catalog_verifier.py::test_m025_article_catalog_rebuild_rejects_duplicate_lookup_key` covers duplicate lookup/index rebuild failures.
- `tests/test_m025_article_catalog_verifier.py::test_m025_article_catalog_verifier_rejects_unsafe_index_traversal` covers unsafe catalog-relative paths.
- `tests/test_m025_article_catalog_verifier.py::test_m025_article_catalog_verifier_writes_catalog_readiness_outputs` covers final summary/report/diagnostics generation and planned S02 blocker reporting.

## Observability Impact
- `run-summary.json` records article, variant, loader, rebuild, network, blocker, and safety counts for S02 handoff.
- `diagnostics.jsonl` records machine-readable index, article, and source-variant readiness rows without raw payloads or vectors.
- `catalog-report.md` provides a human-readable handoff stating primary lightweight variants, preserved PDF fallbacks, idempotency evidence, and S02 blockers.
