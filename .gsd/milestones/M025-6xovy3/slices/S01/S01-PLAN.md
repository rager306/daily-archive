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

## Threat Surface

## Q3 Exploitability Findings

Verdict: **flag** because the slice is local/offline in normal replay, but it creates a durable catalog and verifier that will read paths, URLs, article metadata, raw HTML/PDF/BibTeX, and loader summaries.

### Abuse scenarios to guard
- **Path traversal / arbitrary file reads or writes:** catalog fields such as `source_code`, `coarse_topic_code`, `article_key`, variant paths, `--catalog`, `--index`, `--selection`, `--write-*` arguments, and rebuild traversal could be tampered with to escape `data/article_catalog/` or overwrite unrelated files.
- **Index poisoning / replay drift:** stale or malicious `index.json` entries could point selection lookups at the wrong article record or source variant unless T03/T05 enforce index consistency, duplicate lookup-key checks, title/path/source/topic/URL drift diagnostics, and index-lookup-only replay.
- **SSRF / unbounded network acquisition:** T04 capture may fetch arXiv/blog sources. Acquisition must use allowlisted schemes/hosts or explicit source descriptors, bounded timeouts/size limits, and no implicit network refresh during tests or replay.
- **Malicious document handling:** captured HTML/PDF/BibTeX are untrusted inputs and may contain oversized payloads, script tags, embedded files, pathological PDFs, hostile filenames, or prompt-injection-like text that should remain data, not executable instructions.
- **Raw payload leakage:** loader events, diagnostics, run summaries, and reports must remain metadata-only and redacted; raw paper text, PDF bytes, HTML bodies, secrets, tokens, vectors/embeddings, and large payload snippets must not be copied into metadata artifacts.

### Trust boundaries
- CLI flags and JSON catalog/index/selection records are untrusted inputs before validation.
- Network source responses are untrusted and must be captured only into controlled catalog artifact paths with checksums.
- Loader/parser outputs are untrusted derived metadata until schema/redaction checks pass.
- The slice must not write production LadybugDB, enable KG import, or claim semantic readiness.

### Required retest focus
- Negative tests for path components containing `..`, absolute paths, symlinks, duplicate keys, title/path drift, and canonical URL drift.
- Redaction tests for loader events/summaries and report outputs.
- No-network replay tests proving tests/parser baseline read local artifacts only and report refresh-needed instead of fetching implicitly.

## Requirement Impact

## Q4 Requirement Impact Findings

Verdict: **pass** with explicit retest scope.

### Existing requirements touched
- **R024** — staged real-article KG behavior validation: S01 advances the prerequisite smoke-corpus foundation but does not satisfy 10/20-document or one-week graph-quality validation.
- **R027** — graph-readiness quality contract: S01 establishes catalog/source/profile/provenance inputs that later parser/chunk readiness checks depend on, while avoiding positive graph-readiness claims.
- **R029** — import-ready typed chunk package: S01 prepares stable article/source identity and local source provenance for later chunk package validation; no import-ready package is claimed in S01.
- **R030** — source artifact preservation: S01 directly preserves raw/source variants, checksums, PDF fallback sources, and metadata-safe manifests, so the existing validated source-preservation contract must be rechecked.
- **R036** — replay/audit provenance: S01 adds deterministic local catalog/index/selection/run artifacts and should preserve command/input/output/hash/lineage evidence for reproducible replay.
- **R040** — infrastructure safety: S01 must keep any acquisition/capture local-first, bounded, researched/safety-wrapped, and separate from production writes or external orchestration.

### Requirements to re-test after shipping S01
- Re-run catalog schema/fixture validation for `article-catalog.v00.01`, `article-catalog-index.v00.01`, and `article.v00.01`.
- Re-run source-preservation regression checks covering checksums, source variant roles/formats/capabilities, PDF fallback preservation, metadata/BibTeX handling, and forbidden raw payload keys.
- Re-run redaction checks for loader events, summaries, diagnostics, rebuild reports, and final catalog readiness report.
- Re-run index consistency checks: index required, title presence, idempotent rebuild, duplicate lookup keys, path/source/topic/canonical URL drift, and normal lookup through `data/article_catalog/index.json` only.
- Re-run no-network replay checks proving parser/chunking baseline consumes local catalog artifacts and reports refresh-needed rather than fetching implicitly.
- Re-run no-import/no-write guards: `kg_import_allowed=false`, production LadybugDB writes blocked, no embeddings/vectors/tokens/raw payloads in metadata outputs.

### Decisions to revisit only if scope expands
- If T04 acquisition becomes live-network automation rather than one-time controlled capture, revisit the local-first/no-network completion boundary and add explicit host allowlists, size limits, and retry/rate constraints.
- If S01 outputs are later used for graph import or semantic readiness claims, revisit the M025 preprocessing-only/no graph-import decision before proceeding.

## Proof Level

- This slice proves: Contract plus operational local catalog and index integrity proof on selected real articles.

## Integration Closure

S02 Current Pipeline Baseline must consume the M025 selection over reusable article catalog entries, not live URLs. Later slices replay the same catalog entries and compare against the S02 baseline. The catalog distinguishes source selection, initial index creation, explicit index rebuild, raw source capture, local loader outcome, and later parser/chunking artifacts.

## Verification

- Adds catalog-level and article-level diagnostics: source provider, coarse topic code, article key, title, source variant role/format/capability, initial index state, index rebuild report, index drift diagnostics, capture status, checksum, loader outcome, fallback reason, and safety flags. Tests and pipeline runs must report when they would need network refresh instead of fetching implicitly.

## Tasks

- [x] **T01: Restored T01 completion after splitting S01 index creation and rebuild tasks; catalog, index, and article schema fixtures remain verified.** `est:small`
  Define and test the reusable article catalog contract. The contract must include `article-catalog.v00.01`, `article-catalog-index.v00.01`, `article.v00.01`, source classes, coarse topic codes, source variants, HTML-first/PDF-preserved source strategy, no-network test policy, and fail-closed safety flags. Include fixture entries for the selected articles so the contract is executable before acquisition code is added.
  - Files: `tests/test_article_catalog_schema.py`, `tests/fixtures/article_catalog_v00_01/`
  - Verify: uv run pytest tests/test_article_catalog_schema.py -q
uv run ruff check tests/test_article_catalog_schema.py

- [x] **T02: Created the M025 reusable article catalog scaffold, initial index, schemas, corpus selection, and local-only verifier.** `est:medium`
  Implement the durable catalog scaffold and initial selection writer for the M025 mixed-source corpus. Create local catalog directories using `source_code/coarse_topic_code/article_key`, write `data/article_catalog/catalog.json`, create the initial `data/article_catalog/index.json` from the fixture seed, and create `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json`. This task proves the CLI/verifier can create the first index as part of scaffold initialization; it must not yet rely on rebuilding from discovered article records.
  - Files: `data/article_catalog/`, `data/article_corpora/`, `scripts/verify_m025_article_catalog.py`
  - Verify: uv run python scripts/verify_m025_article_catalog.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --validate-only --require-index --check-index-titles

- [x] **T03: Added an explicit deterministic article catalog index rebuild path and proved the rebuilt index is idempotent.** `est:medium`
  Add the explicit index rebuild path and prove it is idempotent. Implement the catalog indexer as a bounded deterministic metadata projection: traverse only canonical `data/article_catalog/{source_code}/{coarse_topic_code}/{article_key}/article.json` records, reject paths that resolve outside the catalog root, normalize path separators to `/`, sort entries/maps deterministically, and write `index.json` atomically via same-directory temp file plus replace. The rebuild must compare the rebuilt projection to the existing index and detect stale/missing entries, path drift, title drift, source/topic drift, canonical URL drift, duplicate lookup keys, malformed article records, and unsafe traversal attempts. Normal lookup must use the index; full tree traversal is allowed only for explicit rebuild/refresh commands. Add or plan a static guard using ast-grep/tree-sitter or an equivalent AST-aware test to prevent future broad catalog scans outside the rebuild function; ast-grep/tree-sitter is a development-time code policy tool here, not a runtime JSON indexing dependency.
  - Files: `scripts/verify_m025_article_catalog.py`, `data/article_catalog/`, `tests/test_article_catalog_schema.py`
  - Verify: uv run python scripts/verify_m025_article_catalog.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --rebuild-index --write-index data/article_catalog/index.json --write-index-report data/article_catalog/index-rebuild-report.json --write-diagnostics data/article_catalog/index-rebuild-diagnostics.jsonl --check-index-idempotent --check-index-titles --check-safe-traversal --check-duplicate-lookups --check-index-lookup-only

- [x] **T04: Captured the selected M025 source variants into the catalog with checksums and refreshed the deterministic index projection.** `est:medium`
  Capture the selected raw source variants once into the reusable catalog. For arXiv entries, capture available HTML/abs metadata and PDF variants; for the PageIndex blog entry, capture HTML and the provided BibTeX citation. Compute checksums and update article records with capture status. After capture, rerun the explicit index rebuild so title/path/URL/source metadata stays synchronized. Do not parse or chunk yet.
  - Files: `data/article_catalog/`, `scripts/verify_m025_article_catalog.py`
  - Verify: uv run python scripts/verify_m025_article_catalog.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --require-captured-sources --check-checksums --rebuild-index --check-index-idempotent

- [x] **T05: Replayed the local article loader over captured catalog variants and persisted redacted loader events and summaries for all five selected articles.** `est:medium`
  Run the existing local article loader over captured catalog variants and write loader events/summaries back under each article entry. Confirm HTML variants load as text-like sources, PDFs are classified as metadata-only current loader outcomes while remaining content-bearing fallback variants, and BibTeX/metadata variants are treated safely. Loader replay must resolve article paths through `data/article_catalog/index.json`, not by scanning the full tree.
  - Files: `scripts/verify_m025_article_catalog.py`, `data/article_catalog/`
  - Verify: uv run python scripts/verify_m025_article_catalog.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --require-loader-events --check-redaction --check-index-lookup-only

- [x] **T06: Finalized the M025 S01 catalog readiness report, run summary, and diagnostics for S02 handoff.** `est:small`
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
