# S02: PageIndex document navigation

**Goal:** Implement the PageIndexNode hierarchy and navigation primitives for intra-paper structure using the S01 full-text ingestion boundary.
**Demo:** After this: a fixture paper can be navigated as an ordered PageIndexNode tree with parent child and NEXT links.

## Must-Haves

- PageIndexNode records preserve title, level, order, parent, children, NEXT relation, source path, and provenance.
- Tests can locate abstract, method, conclusion, and fallback sections in fixtures.
- Navigation returns stable Paper -> PageIndexNode paths and ordered NEXT traversal.
- Structure-quality validation reports completeness/order issues without requiring network or PDF access.

## Proof Level

- This slice proves: This slice proves: deterministic PageIndexNode hierarchy and navigation primitives over fixture full text.

## Integration Closure

Consumes S01 `FullTextIngestionResult` and produces a deterministic PageIndex tree that S03 SemanticChunk and EvidencePath work can attach to. Does not change M001 cron CLI behavior or introduce LadybugDB writes.

## Verification

- PageIndex construction returns validation diagnostics for missing headings, order gaps, empty sections, fallback section creation, parent/child consistency, and NEXT-link consistency so future agents can diagnose bad document structure without inspecting parser internals.

## Tasks

- [ ] **T01: Add PageIndex contract tests and fallback fixture** `est:45m`
  Create red contract tests for PageIndex construction over the S01 structured markdown fixture and a malformed/no-heading fallback fixture. Define expected `PageIndexNode` fields, deterministic IDs, parent/child relationships, ordered NEXT traversal, stable Paper -> node paths, and validation diagnostics. Done when the new tests fail for missing `arxiv_archive.page_index` while S01 ingestion tests still pass.
  - Files: `tests/fixtures/page_index/no_headings.txt`, `tests/test_page_index.py`
  - Verify: uv run pytest tests/test_page_index.py -q

- [ ] **T02: Implement PageIndex document construction** `est:1h 15m`
  Implement `src/arxiv_archive/page_index.py` with typed `PageIndexNode`, `PageIndexDocument`, and construction from `FullTextIngestionResult`. Parse markdown headings into a hierarchy with deterministic node IDs and ordered relationships; create an explicit fallback root/section when no headings exist. Done when the initial PageIndex contract tests pass.
  - Files: `src/arxiv_archive/page_index.py`, `tests/test_page_index.py`
  - Verify: uv run pytest tests/test_page_index.py tests/test_full_text_ingestion.py -q

- [ ] **T03: Add PageIndex navigation and validation helpers** `est:1h`
  Add navigation helpers and tests for locating sections by title, computing stable Paper -> PageIndexNode paths, walking NEXT links, and validating parent/child/NEXT consistency. Keep helpers pure and fixture-local. Done when downstream S03 can attach chunks to stable node IDs without inspecting parser internals.
  - Files: `src/arxiv_archive/page_index.py`, `tests/test_page_index.py`
  - Verify: uv run pytest tests/test_page_index.py tests/test_full_text_ingestion.py -q

- [ ] **T04: Run S02 quality gates and regression smoke** `est:30m`
  Run final S02 regression gates: PageIndex tests, S01 ingestion tests, relevant analysis regression, Ruff on touched files, and a no-CLI-change smoke check. Record known limitations for S03, especially simple markdown parsing and no chunking yet. Done when S02 is ready for closeout.
  - Files: `src/arxiv_archive/page_index.py`, `tests/test_page_index.py`
  - Verify: uv run pytest tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q

## Files Likely Touched

- tests/fixtures/page_index/no_headings.txt
- tests/test_page_index.py
- src/arxiv_archive/page_index.py
