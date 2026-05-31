---
id: T01
parent: S01
milestone: M025-6xovy3
key_files:
  - tests/test_article_catalog_schema.py
  - tests/fixtures/article_catalog_v00_01/catalog.json
  - tests/fixtures/article_catalog_v00_01/article_catalog/index.json
  - tests/fixtures/article_catalog_v00_01/corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json
  - tests/fixtures/article_catalog_v00_01/article_catalog/arxiv/cs-ai/2512.24601/article.json
  - tests/fixtures/article_catalog_v00_01/article_catalog/arxiv/cs-ai/2605.28617v1/article.json
  - tests/fixtures/article_catalog_v00_01/article_catalog/arxiv/cs-cv/2605.26525v1/article.json
  - tests/fixtures/article_catalog_v00_01/article_catalog/arxiv/cs-cl/2507.19457/article.json
  - tests/fixtures/article_catalog_v00_01/article_catalog/company_blog/cs-ir/pageindex_zhang2025pageindex/article.json
  - .gsd/milestones/M025-6xovy3/slices/S01/S01-PLAN.md
key_decisions:
  - Catalog index v00.01 is part of the T01 contract and must include article titles for CLI display/search.
  - Catalog CLI/verification must use `data/article_catalog/index.json` for lookup rather than scanning the full source/topic/article tree during normal test/pipeline execution.
duration: 
verification_result: passed
completed_at: 2026-05-31T14:04:17.172Z
blocker_discovered: false
---

# T01: Restored T01 completion after S01 replanning; catalog, index, and article schema fixtures remain verified.

**Restored T01 completion after S01 replanning; catalog, index, and article schema fixtures remain verified.**

## What Happened

Restored completion state for the T01 schema contract after S01 replanning. The current T01 artifacts define executable fixture contracts for `article-catalog.v00.01`, `article-catalog-index.v00.01`, and `article.v00.01`. The selected M025 mixed-source corpus includes four arXiv RLM/DSPy-related articles and one PageIndex company-blog article with BibTeX metadata. The catalog index contract includes article key, citation key, canonical URL, source code, coarse topic code, and title lookups, and each index entry mirrors the article title from `article.json`. The contract also preserves the HTML-first/PDF-preserved source strategy and fail-closed safety flags.

## Verification

Fresh verification passed after the pre-auto cleanup: `uv run pytest tests/test_article_catalog_schema.py -q && uv run ruff check tests/test_article_catalog_schema.py` reported 8 passed and all ruff checks passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_article_catalog_schema.py -q && uv run ruff check tests/test_article_catalog_schema.py` | 0 | ✅ pass — 8 passed in 0.12s; All checks passed | 645ms |

## Deviations

S01 was replanned during pre-auto cleanup to synchronize the T02 index/title requirement, which reset T01 status to pending. No T01 implementation changed after verification; this completion restores the correct task state.

## Known Issues

The fixture contract is schema-level and selection-level only: raw source files are not captured yet, checksums are still pending/null, the real `data/article_catalog/index.json` writer is not implemented yet, and loader execution over local catalog variants is planned for T03/T04.

## Files Created/Modified

- `tests/test_article_catalog_schema.py`
- `tests/fixtures/article_catalog_v00_01/catalog.json`
- `tests/fixtures/article_catalog_v00_01/article_catalog/index.json`
- `tests/fixtures/article_catalog_v00_01/corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json`
- `tests/fixtures/article_catalog_v00_01/article_catalog/arxiv/cs-ai/2512.24601/article.json`
- `tests/fixtures/article_catalog_v00_01/article_catalog/arxiv/cs-ai/2605.28617v1/article.json`
- `tests/fixtures/article_catalog_v00_01/article_catalog/arxiv/cs-cv/2605.26525v1/article.json`
- `tests/fixtures/article_catalog_v00_01/article_catalog/arxiv/cs-cl/2507.19457/article.json`
- `tests/fixtures/article_catalog_v00_01/article_catalog/company_blog/cs-ir/pageindex_zhang2025pageindex/article.json`
- `.gsd/milestones/M025-6xovy3/slices/S01/S01-PLAN.md`
