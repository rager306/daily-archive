# S01: Smoke Corpus Freeze

**Goal:** Freeze the M025 reusable article catalog foundation with article-catalog.v00.01 and article.v00.01, register the selected mixed-source RLM/DSPy/PageIndex articles, create and rebuild the catalog index explicitly, capture raw source variants once into the catalog, and prove the existing loader can classify those local variants before S02 parser/chunking baseline begins.
**Demo:** After this: a fixed 5 article corpus exists with local source artifacts, catalog index entries, checksums, expected profiles, and an integrity validator so every later slice uses the same real inputs.

## Must-Haves

- `article-catalog.v00.01`, `article-catalog-index.v00.01`, and `article.v00.01` are defined and covered by tests.
- The reusable catalog hierarchy uses `source_code/coarse_topic_code/article_key` and supports at least `arxiv`, `company_blog`, `personal_blog`, and `nature` source classes.
- The selected corpus includes four arXiv RLM/DSPy-related papers plus the PageIndex company-blog article with BibTeX metadata.
- Each selected article has source variants, a primary source strategy, expected profile fields, and an index entry with title.
- The CLI/verifier first creates an initial index, then supports explicit index rebuild from article records, then validates index/catalog/selection consistency.
- Normal CLI lookup uses `data/article_catalog/index.json`; full tree traversal is allowed only for explicit rebuild/refresh commands.
- HTML/Markdown-like lightweight sources are preferred for normal preprocessing when available, while PDFs are captured immediately and preserved as fallback/evidence sources.
- Capture/acquisition may fetch network sources, but tests and parser/chunking replay must read only local catalog artifacts.
- The existing loader is run against captured local variants and writes metadata-only loader events/summaries without raw payload leakage.
- S01 does not run parser/chunking; S02 owns current pipeline baseline over the local catalog.

## Proof Level

- This slice proves: Contract plus operational local catalog and index integrity proof on selected real articles.

## Integration Closure

S02 Current Pipeline Baseline must consume the M025 selection over reusable article catalog entries, not live URLs. Later slices replay the same catalog entries and compare against the S02 baseline. The catalog distinguishes source selection, initial index creation, explicit index rebuild, raw source capture, local loader outcome, and later parser/chunking artifacts.

## Verification

- Adds catalog-level and article-level diagnostics: source provider, coarse topic code, article key, title, source variant role/format/capability, initial index state, index rebuild report, index drift diagnostics, capture status, checksum, loader outcome, fallback reason, and safety flags. Tests and pipeline runs must report when they would need network refresh instead of fetching implicitly.

## Tasks

- [x] **T01: Define article catalog schema contract** `est:small`
  Define and test the reusable article catalog contract. The contract must include `article-catalog.v00.01`, `article-catalog-index.v00.01`, `article.v00.01`, source classes, coarse topic codes, source variants, HTML-first/PDF-preserved source strategy, no-network test policy, and fail-closed safety flags. Include fixture entries for the selected articles so the contract is executable before acquisition code is added.
  - Files: `tests/test_article_catalog_schema.py`, `tests/fixtures/article_catalog_v00_01/`
  - Verify: uv run pytest tests/test_article_catalog_schema.py -q
uv run ruff check tests/test_article_catalog_schema.py

- [x] **T02: Create catalog scaffold and initial index** `est:medium`
  Implement the durable catalog scaffold and initial selection writer for the M025 mixed-source corpus. Create local catalog directories using `source_code/coarse_topic_code/article_key`, write `data/article_catalog/catalog.json`, create the initial `data/article_catalog/index.json` from the fixture seed, and create `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json`. This task proves the CLI/verifier can create the first index as part of scaffold initialization; it must not yet rely on rebuilding from discovered article records.
  - Files: `data/article_catalog/`, `data/article_corpora/`, `scripts/verify_m025_article_catalog.py`
  - Verify: uv run python scripts/verify_m025_article_catalog.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --validate-only --require-index --check-index-titles

- [x] **T03: Rebuild catalog index from article records** `est:medium`
  Add the explicit index rebuild path and prove it is idempotent. The CLI/verifier must rebuild `data/article_catalog/index.json` from the article records under `data/article_catalog/{source_code}/{coarse_topic_code}/{article_key}/article.json`, compare the rebuilt output to the existing index, and detect stale/missing entries, path drift, title drift, source/topic drift, canonical URL drift, and duplicate lookup keys. Normal lookup must use the index; full tree traversal is allowed only for this explicit rebuild/refresh command.
  - Files: `scripts/verify_m025_article_catalog.py`, `data/article_catalog/`
  - Verify: uv run python scripts/verify_m025_article_catalog.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --rebuild-index --write-index data/article_catalog/index.json --write-index-report data/article_catalog/index-rebuild-report.json --write-diagnostics data/article_catalog/index-rebuild-diagnostics.jsonl --check-index-idempotent --check-index-titles

- [x] **T04: Capture selected raw source variants** `est:medium`
  Capture the selected raw source variants once into the reusable catalog. For arXiv entries, capture available HTML/abs metadata and PDF variants; for the PageIndex blog entry, capture HTML and the provided BibTeX citation. Compute checksums and update article records with capture status. After capture, rerun the explicit index rebuild so title/path/URL/source metadata stays synchronized. Do not parse or chunk yet.
  - Files: `data/article_catalog/`, `scripts/verify_m025_article_catalog.py`
  - Verify: uv run python scripts/verify_m025_article_catalog.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --require-captured-sources --check-checksums --rebuild-index --check-index-idempotent

- [x] **T05: Run loader over local catalog variants** `est:medium`
  Run the existing local article loader over captured catalog variants and write loader events/summaries back under each article entry. Confirm HTML variants load as text-like sources, PDFs are classified as metadata-only current loader outcomes while remaining content-bearing fallback variants, and BibTeX/metadata variants are treated safely. Loader replay must resolve article paths through `data/article_catalog/index.json`, not by scanning the full tree.
  - Files: `scripts/verify_m025_article_catalog.py`, `data/article_catalog/`
  - Verify: uv run python scripts/verify_m025_article_catalog.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --require-loader-events --check-redaction --check-index-lookup-only

- [ ] **T06: Finalize catalog readiness report** `est:small`
  Finalize the S01 catalog readiness report and machine-readable run summary. The report must state which source variants are captured, which are primary for lightweight preprocessing, which PDFs are preserved as fallback, whether the index was rebuilt and checked for idempotency, and whether any article is blocked before S02 baseline.
  - Files: `scripts/verify_m025_article_catalog.py`, `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/`
  - Verify: uv run python scripts/verify_m025_article_catalog.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --require-loader-events --check-redaction --check-index-idempotent --write-summary data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/run-summary.json --write-diagnostics data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/diagnostics.jsonl --write-report data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/catalog-report.md

## Files Likely Touched

- tests/test_article_catalog_schema.py
- tests/fixtures/article_catalog_v00_01/
- data/article_catalog/
- data/article_corpora/
- scripts/verify_m025_article_catalog.py
- data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/
