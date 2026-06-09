# S03: Semantic chunks and evidence paths

**Goal:** Add SemanticChunk and EvidencePath models with deterministic IDs and traceability over S02 PageIndex documents.
**Demo:** After this: PageIndex nodes can own SemanticChunk records and EvidencePath objects can point from paper to section to chunk.

## Must-Haves

- SemanticChunk records attach to PageIndexNode.
- EvidencePath can represent Paper -> PageIndexNode -> SemanticChunk.
- Chunk IDs are deterministic and idempotent.
- Tests cover boundary ordering, empty sections, malformed/fallback sections, and evidence-path validation errors.

## Threat Surface

## Q3 Findings

- S03 has a narrow evidence-substrate scope: SemanticChunk records and EvidencePath references over already-tested PageIndex documents.
- The plan is test-first and explicitly avoids embeddings, LLMs, DSPy, claims/entities, and LadybugDB writes.
- Integration boundaries are clear: S03 consumes S02 PageIndexNode IDs and provides stable references for S04/S05.
- Risk is controlled by deterministic IDs, fixture-local chunking, and data-returned validation diagnostics.

Verdict: pass.

## Requirement Impact

## Q4 Findings

- Every task has an explicit verification command, ending in evidence/PageIndex/ingestion/analysis/CLI regression smoke.
- Observability requirements are concrete: chunk boundaries, chunk strategy, character spans, source node ids, provenance, and evidence-path validation diagnostics.
- Failure modes are planned before implementation: empty sections, fallback sections, missing nodes/chunks, paper mismatch, and invalid node/chunk attachment.
- The plan preserves the user decision that DSPy remains gated until metrics and benchmark fixtures are verified.

Verdict: pass.

## Proof Level

- This slice proves: model and fixture tests

## Integration Closure

Consumes S02 PageIndexDocument and PageIndexNode IDs, producing chunk and evidence-path records that S04 claim/entity/relation contracts and S05 LadybugDB schema expansion can reference without inspecting parser internals. Does not create embeddings, run LLMs, or write LadybugDB records.

## Verification

- Chunking and evidence-path functions expose chunk boundaries, chunking strategy, character spans, source node ids, provenance, and validation diagnostics as code-readable fields.

## Tasks

- [x] **T01: Added red SemanticChunk and EvidencePath contract tests for S03.** `est:45m`
  Create red contract tests for `SemanticChunk`, `EvidencePath`, deterministic chunk IDs, chunk ordering, PageIndexNode attachment, empty/fallback section behavior, and validation diagnostics. Tests should consume S01/S02 fixtures through `ingest_full_text()` and `build_page_index()` so the contract is vertical. Done when the new tests fail for missing semantic/evidence implementation while PageIndex tests still pass.
  - Files: `tests/test_evidence_paths.py`
  - Verify: uv run pytest tests/test_evidence_paths.py -q

- [x] **T02: Implemented deterministic SemanticChunk construction and the initial EvidencePath boundary.** `est:1h 15m`
  Implement `src/arxiv_archive/evidence.py` with `SemanticChunk`, deterministic chunking from `PageIndexDocument`, chunk provenance, chunk order, and character spans. Keep chunking simple and deterministic: section-level or paragraph-aware chunks over PageIndexNode text, no embeddings or LLM calls. Done when initial SemanticChunk contract tests pass together with PageIndex tests.
  - Files: `src/arxiv_archive/evidence.py`, `tests/test_evidence_paths.py`
  - Verify: uv run pytest tests/test_evidence_paths.py tests/test_page_index.py -q

- [x] **T03: Added EvidencePath validation coverage for missing chunks, node-path mismatch, and fallback evidence.** `est:1h`
  Implement `EvidencePath` construction and validation helpers that prove each path references an existing paper id, PageIndexNode id, and SemanticChunk id. Add tests for valid paths, missing node, missing chunk, paper mismatch, and fallback-section evidence. Done when S04 can reference evidence paths without revalidating PageIndex internals.
  - Files: `src/arxiv_archive/evidence.py`, `tests/test_evidence_paths.py`
  - Verify: uv run pytest tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py -q

- [x] **T04: Ran final S03 evidence-path regression, lint, diagnostics, and public CLI help smoke successfully.** `est:30m`
  Run final S03 regression gates: evidence-path tests, PageIndex tests, S01 ingestion tests, analysis regression, CLI contract smoke, Ruff on touched files, and public module help smoke. Record limitations for S04 and S05: no claims/entities, no embeddings, no LadybugDB persistence, and simple deterministic chunking only. Done when S03 is ready for closeout and requirements restoration can follow.
  - Files: `src/arxiv_archive/evidence.py`, `tests/test_evidence_paths.py`
  - Verify: uv run pytest tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q

## Files Likely Touched

- tests/test_evidence_paths.py
- src/arxiv_archive/evidence.py
