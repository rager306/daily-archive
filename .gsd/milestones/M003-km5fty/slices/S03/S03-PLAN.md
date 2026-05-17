# S03: Semantic chunks and evidence paths

**Goal:** Add SemanticChunk and EvidencePath models with deterministic IDs and traceability over S02 PageIndex documents.
**Demo:** After this: PageIndex nodes can own SemanticChunk records and EvidencePath objects can point from paper to section to chunk.

## Must-Haves

- SemanticChunk records attach to PageIndexNode.
- EvidencePath can represent Paper -> PageIndexNode -> SemanticChunk.
- Chunk IDs are deterministic and idempotent.
- Tests cover boundary ordering, empty sections, malformed/fallback sections, and evidence-path validation errors.

## Proof Level

- This slice proves: model and fixture tests

## Integration Closure

Consumes S02 PageIndexDocument and PageIndexNode IDs, producing chunk and evidence-path records that S04 claim/entity/relation contracts and S05 LadybugDB schema expansion can reference without inspecting parser internals. Does not create embeddings, run LLMs, or write LadybugDB records.

## Verification

- Chunking and evidence-path functions expose chunk boundaries, chunking strategy, character spans, source node ids, provenance, and validation diagnostics as code-readable fields.

## Tasks

- [x] **T01: Add SemanticChunk and EvidencePath contract tests** `est:45m`
  Create red contract tests for `SemanticChunk`, `EvidencePath`, deterministic chunk IDs, chunk ordering, PageIndexNode attachment, empty/fallback section behavior, and validation diagnostics. Tests should consume S01/S02 fixtures through `ingest_full_text()` and `build_page_index()` so the contract is vertical. Done when the new tests fail for missing semantic/evidence implementation while PageIndex tests still pass.
  - Files: `tests/test_evidence_paths.py`
  - Verify: uv run pytest tests/test_evidence_paths.py -q

- [x] **T02: Implement deterministic SemanticChunk construction** `est:1h 15m`
  Implement `src/arxiv_archive/evidence.py` with `SemanticChunk`, deterministic chunking from `PageIndexDocument`, chunk provenance, chunk order, and character spans. Keep chunking simple and deterministic: section-level or paragraph-aware chunks over PageIndexNode text, no embeddings or LLM calls. Done when initial SemanticChunk contract tests pass together with PageIndex tests.
  - Files: `src/arxiv_archive/evidence.py`, `tests/test_evidence_paths.py`
  - Verify: uv run pytest tests/test_evidence_paths.py tests/test_page_index.py -q

- [x] **T03: Implement EvidencePath validation helpers** `est:1h`
  Implement `EvidencePath` construction and validation helpers that prove each path references an existing paper id, PageIndexNode id, and SemanticChunk id. Add tests for valid paths, missing node, missing chunk, paper mismatch, and fallback-section evidence. Done when S04 can reference evidence paths without revalidating PageIndex internals.
  - Files: `src/arxiv_archive/evidence.py`, `tests/test_evidence_paths.py`
  - Verify: uv run pytest tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py -q

- [ ] **T04: Run S03 quality gates and regression smoke** `est:30m`
  Run final S03 regression gates: evidence-path tests, PageIndex tests, S01 ingestion tests, analysis regression, CLI contract smoke, Ruff on touched files, and public module help smoke. Record limitations for S04 and S05: no claims/entities, no embeddings, no LadybugDB persistence, and simple deterministic chunking only. Done when S03 is ready for closeout and requirements restoration can follow.
  - Files: `src/arxiv_archive/evidence.py`, `tests/test_evidence_paths.py`
  - Verify: uv run pytest tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q

## Files Likely Touched

- tests/test_evidence_paths.py
- src/arxiv_archive/evidence.py
