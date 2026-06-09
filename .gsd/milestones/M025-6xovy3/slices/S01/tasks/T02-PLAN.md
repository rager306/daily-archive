---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T02: Created the M025 reusable article catalog scaffold, initial index, schemas, corpus selection, and local-only verifier.

Implement the durable catalog scaffold and initial selection writer for the M025 mixed-source corpus. Create local catalog directories using `source_code/coarse_topic_code/article_key`, write `data/article_catalog/catalog.json`, create the initial `data/article_catalog/index.json` from the fixture seed, and create `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json`. This task proves the CLI/verifier can create the first index as part of scaffold initialization; it must not yet rely on rebuilding from discovered article records.

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

uv run python scripts/verify_m025_article_catalog.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --validate-only --require-index --check-index-titles

## Observability Impact

Creates stable local paths and the initial machine-readable catalog index so future CLI commands can resolve and display articles without expensive or ambiguous tree scans.
