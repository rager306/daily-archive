# S03: Structure aware chunk model implementation

**Goal:** Implement deterministic structure-aware chunk construction that emits typed parent-child chunks with canonical normalized-Markdown source spans, route eligibility, and contract-shaped package output suitable for later benchmark and import rehearsal gates.
**Demo:** After this slice, a structure-aware chunk model emits typed parent-child chunks with provenance and route eligibility.

## Must-Haves

- Structure-aware chunks preserve deterministic parent-child hierarchy from normalized Markdown sections and block elements.
- Chunk source spans use canonical normalized Markdown coordinates, not `semantic_chunk_text` coordinates.
- Chunks are typed by content class such as section, paragraph, table, figure, equation, reference, and retrieval context.
- Route eligibility distinguishes claim/method/entity/relation/table/citation/metadata/retrieval-only/excluded uses.
- Output packages validate against the S01 contract without raw text, embeddings, vectors, or production LadybugDB writes.
- The S02 baseline path remains runnable so S05 can compare before/after quality.

## Proof Level

- This slice proves: Automated tests for span correctness, hierarchy, route/state assignment, redaction, and contract validation; a bounded gold-corpus dry run; independent artifact review before slice closure.

## Integration Closure

S03 consumes S01's import-ready chunk contract and S02's baseline evidence. It should produce a new structure-aware chunking module and tests that can build contract-shaped packages without production KG writes. Downstream S04 can add deterministic annotation sidecars; S05 can benchmark improved vs baseline quality; S06 can rehearse isolated import.

## Verification

- S03 should emit redacted package diagnostics with counts by chunk type, route, state, source-span coverage, parent-child coverage, and safety flags. Any markdown review samples must remain separate from machine JSON/JSONL outputs.

## Tasks

- [x] **T01: Define structure aware chunking model** `est:medium`
  Define the S03 structure-aware chunking module interface and core dataclasses for structural elements, chunks, source spans, hierarchy links, route eligibility, and package output. Keep the API deterministic and independent of LLM calls or production KG writes.
  - Files: `src/arxiv_archive/structure_aware_chunking.py`, `tests/test_structure_aware_chunking.py`
  - Verify: uv run pytest tests/test_structure_aware_chunking.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py

- [ ] **T02: Parse markdown structure with canonical spans** `est:large`
  Implement deterministic parsing from canonical normalized Markdown into structural elements with absolute character spans and parent-child hierarchy. Cover headings, paragraphs, references, tables, figures/captions, and equation-like blocks where detectable without LLMs.
  - Files: `src/arxiv_archive/structure_aware_chunking.py`, `tests/test_structure_aware_chunking.py`
  - Verify: uv run pytest tests/test_structure_aware_chunking.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py

- [ ] **T03: Assign routes states and refusal reasons** `est:large`
  Assign deterministic chunk types, routes, quality states, allowed/excluded uses, and refusal reasons from structural element classes. Ensure references, administrative/front-matter, tables, figures, equations, method sections, and retrieval-only prose are routed conservatively.
  - Files: `src/arxiv_archive/structure_aware_chunking.py`, `tests/test_structure_aware_chunking.py`
  - Verify: uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py

- [ ] **T04: Validate structure aware packages on gold corpus** `est:large`
  Build S01 contract-shaped packages from structure-aware chunks and validate them with the existing import contract validator. Add a CLI or callable dry-run path that writes redacted structure-aware package diagnostics for the gold corpus without writing production KG data.
  - Files: `src/arxiv_archive/structure_aware_chunking.py`, `tests/test_structure_aware_chunking.py`, `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-summary.json`, `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-package-diagnostics.jsonl`
  - Verify: uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-summary.json && test -s .gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-package-diagnostics.jsonl

- [ ] **T05: Report and review structure aware implementation** `est:medium`
  Write the S03 implementation report and run independent review over the structure-aware package outputs. The report must compare against the S02 baseline boundary without claiming final KG import readiness or production persistence.
  - Files: `.gsd/milestones/M005-dlko4z/slices/S03/structure-aware-implementation-report.md`, `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-review-summary.md`
  - Verify: uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S03/structure-aware-implementation-report.md && test -s .gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-review-summary.md

## Files Likely Touched

- src/arxiv_archive/structure_aware_chunking.py
- tests/test_structure_aware_chunking.py
- .gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-summary.json
- .gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-package-diagnostics.jsonl
- .gsd/milestones/M005-dlko4z/slices/S03/structure-aware-implementation-report.md
- .gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-review-summary.md
