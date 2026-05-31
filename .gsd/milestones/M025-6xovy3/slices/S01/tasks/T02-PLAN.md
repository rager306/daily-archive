---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T02: Create reusable catalog scaffold

Implement the durable catalog scaffold, catalog index, and selection writer for the M025 mixed-source corpus. Create local catalog directories using `source_code/coarse_topic_code/article_key`, create `data/article_catalog/index.json` as the CLI lookup surface, and create selection metadata that references catalog entries rather than treating milestone artifacts as the raw source of truth. The CLI/verification path must use the index for lookups by article key, citation key, article title, canonical URL, source code, and coarse topic code; it must not scan the full tree during normal test/pipeline execution. Each index article entry must include the article title so CLI list/search results can be useful without opening every `article.json`.

## Inputs

- `tests/fixtures/article_catalog_v00_01/catalog.json`
- `tests/fixtures/article_catalog_v00_01/article_catalog/index.json`
- `tests/fixtures/article_catalog_v00_01/corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json`

## Expected Output

- `data/article_catalog/catalog.json`
- `data/article_catalog/index.json`
- `data/article_catalog/schemas/article-catalog-schema.v00.01.json`
- `data/article_catalog/schemas/article-schema.v00.01.json`
- `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json`

## Verification

uv run python scripts/verify_m025_article_catalog.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --validate-only

## Observability Impact

Creates stable local paths and a machine-readable catalog index so future CLI commands can resolve and display articles without expensive or ambiguous tree scans. Reports missing schema/catalog/index/selection files, stale index entries, missing article titles, and index-to-article path/title drift as typed diagnostics.
