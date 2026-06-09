# S02: PageIndex document navigation

**Goal:** Implement the PageIndexNode hierarchy and navigation primitives for intra-paper structure using the S01 full-text ingestion boundary.
**Demo:** After this: a fixture paper can be navigated as an ordered PageIndexNode tree with parent child and NEXT links.

## Must-Haves

- PageIndexNode records preserve title, level, order, parent, children, NEXT relation, source path, and provenance.
- Tests can locate abstract, method, conclusion, and fallback sections in fixtures.
- Navigation returns stable Paper to PageIndexNode paths and ordered NEXT traversal.
- Structure-quality validation reports completeness and order issues without requiring network or PDF access.

## Threat Surface

## Q3 Findings

- S02 has a narrow structural scope: consume S01 `FullTextIngestionResult` and produce deterministic PageIndex document navigation primitives.
- The plan is test-first: T01 defines PageIndex contracts and fallback fixture before implementation.
- Integration boundaries are explicit: no public CLI changes, no LadybugDB writes, no network/PDF access.
- The slice risk is high because PageIndex structure feeds S03 chunking and later RLM navigation, but tasks isolate the risk through fixture-only parsing, deterministic IDs, validation diagnostics, and final regression smoke.

Verdict: pass.

## Requirement Impact

## Q4 Findings

- Task verification is explicit at each step, ending in PageIndex + ingestion + analysis + CLI contract regression smoke.
- Observability requirements are concrete: validation diagnostics for missing headings, order gaps, fallback section creation, parent/child consistency, and NEXT-link consistency.
- Failure modes are planned before implementation: malformed/no-heading text, missing structure, and broken navigation invariants.
- S02 has a clear handoff to S03: stable PageIndexNode IDs and paths for SemanticChunk/EvidencePath attachment.

Verdict: pass.

## Proof Level

- This slice proves: deterministic PageIndexNode hierarchy and navigation primitives over fixture full text

## Integration Closure

Consumes S01 FullTextIngestionResult and produces a deterministic PageIndex tree that S03 SemanticChunk and EvidencePath work can attach to. Does not change M001 cron CLI behavior or introduce LadybugDB writes.

## Verification

- PageIndex construction returns validation diagnostics for missing headings, order gaps, empty sections, fallback section creation, parent child consistency, and NEXT-link consistency so future agents can diagnose bad document structure without inspecting parser internals.

## Tasks

- [x] **T01: Added red PageIndex contract tests and a fallback no-heading fixture for S02.** `est:45m`
  Create red contract tests for PageIndex construction over the S01 structured markdown fixture and a malformed/no-heading fallback fixture. Define expected `PageIndexNode` fields, deterministic IDs, parent/child relationships, ordered NEXT traversal, stable Paper -> node paths, and validation diagnostics. Done when the new tests fail for missing `arxiv_archive.page_index` while S01 ingestion tests still pass.
  - Files: `tests/fixtures/page_index/no_headings.txt`, `tests/test_page_index.py`
  - Verify: uv run pytest tests/test_page_index.py -q

- [x] **T02: Implemented PageIndex document construction from S01 full-text ingestion results.** `est:1h 15m`
  Implement `src/arxiv_archive/page_index.py` with typed `PageIndexNode`, `PageIndexDocument`, and construction from `FullTextIngestionResult`. Parse markdown headings into a hierarchy with deterministic node IDs and ordered relationships; create an explicit fallback root/section when no headings exist. Done when the initial PageIndex contract tests pass.
  - Files: `src/arxiv_archive/page_index.py`, `tests/test_page_index.py`
  - Verify: uv run pytest tests/test_page_index.py tests/test_full_text_ingestion.py -q

- [x] **T03: Added PageIndex navigation lookup helpers and structural validation diagnostics.** `est:1h`
  Add navigation helpers and tests for locating sections by title, computing stable Paper -> PageIndexNode paths, walking NEXT links, and validating parent/child/NEXT consistency. Keep helpers pure and fixture-local. Done when downstream S03 can attach chunks to stable node IDs without inspecting parser internals.
  - Files: `src/arxiv_archive/page_index.py`, `tests/test_page_index.py`
  - Verify: uv run pytest tests/test_page_index.py tests/test_full_text_ingestion.py -q

- [x] **T04: Ran final S02 PageIndex regression, lint, diagnostics, and public CLI help smoke successfully.** `est:30m`
  Run final S02 regression gates: PageIndex tests, S01 ingestion tests, relevant analysis regression, Ruff on touched files, and a no-CLI-change smoke check. Record known limitations for S03, especially simple markdown parsing and no chunking yet. Done when S02 is ready for closeout.
  - Files: `src/arxiv_archive/page_index.py`, `tests/test_page_index.py`
  - Verify: uv run pytest tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q

## Files Likely Touched

- tests/fixtures/page_index/no_headings.txt
- tests/test_page_index.py
- src/arxiv_archive/page_index.py
