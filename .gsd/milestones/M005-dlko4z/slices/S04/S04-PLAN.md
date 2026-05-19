# S04: Chunk annotation sidecars

**Goal:** Add deterministic annotation sidecars over structure-aware chunks to support routing, review, and benchmark analysis without promoting annotations to KG facts or authorizing import readiness.
**Demo:** After this slice, chunks have deterministic sidecar annotations useful for routing and review, without becoming KG facts.

## Must-Haves

- Deterministic sidecar annotations are attached to structure-aware chunks without LLM calls.
- Annotation records reference existing chunk ids and never include raw chunk text.
- Annotation types capture route hints, structural cues, section role, table/figure/equation/reference flags, and review blockers.
- Every annotation has `promoted_to_fact=false` and is excluded from trusted KG import.
- Dry-run artifacts summarize annotation coverage and warnings with no production KG writes.
- S03 package validation remains green after sidecar annotations are included.

## Proof Level

- This slice proves: Automated tests for annotation schema, redaction, chunk reference resolution, `promoted_to_fact=false`, and dry-run artifacts; independent review deferred to S05 unless a semantic blocker appears.

## Integration Closure

S04 consumes S03 structure-aware chunk package outputs and emits annotation sidecars that remain deterministic, redacted, and non-factual. S05 can benchmark whether these sidecars improve reviewability and route precision. S06 remains blocked until S05 passes.

## Verification

- S04 should write redacted annotation summaries with counts by annotation type, confidence class, route, warning/refusal reason, and `promoted_to_fact=false` for every annotation. No raw text, embeddings, vectors, optimizer traces, or production writes.

## Tasks

- [x] **T01: Define deterministic annotation sidecars** `est:medium`
  Define annotation sidecar dataclasses and contract serialization for deterministic chunk annotations. Include annotation id, paper id, chunk id, method, annotation type, values, confidence class, warnings, and `promoted_to_fact=false`.
  - Files: `src/arxiv_archive/structure_aware_chunking.py`, `tests/test_structure_aware_chunking.py`
  - Verify: uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py

- [x] **T02: Generate sidecars from chunk metadata** `est:large`
  Generate deterministic sidecar annotations from structural chunk metadata, including section role, route hint, structural type, table/figure/equation/reference flags, and review blockers. Do not inspect or persist raw chunk text.
  - Files: `src/arxiv_archive/structure_aware_chunking.py`, `tests/test_structure_aware_chunking.py`
  - Verify: uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py

- [x] **T03: Validate annotation contract boundaries** `est:medium`
  Validate that annotation sidecars satisfy the S01 contract: all chunk references resolve, redaction holds, `promoted_to_fact=false`, and no annotation creates import eligibility. Include negative tests for unresolved chunks, promoted facts, and raw text leakage.
  - Files: `tests/test_structure_aware_chunking.py`, `src/arxiv_archive/structure_aware_chunking.py`
  - Verify: uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py

- [ ] **T04: Run annotation dry run on gold corpus** `est:medium`
  Run the annotation sidecar dry-run over the gold corpus and write redacted annotation summary plus package diagnostics. Confirm annotation counts and warnings are present while all import/no-write safety flags remain false.
  - Files: `src/arxiv_archive/structure_aware_chunking.py`, `tests/test_structure_aware_chunking.py`, `.gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-summary.json`, `.gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-package-diagnostics.jsonl`
  - Verify: uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-summary.json && test -s .gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-package-diagnostics.jsonl

## Files Likely Touched

- src/arxiv_archive/structure_aware_chunking.py
- tests/test_structure_aware_chunking.py
- .gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-summary.json
- .gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-package-diagnostics.jsonl
