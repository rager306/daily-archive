---
id: S03
parent: M003-km5fty
milestone: M003-km5fty
provides:
  - A tested `arxiv_archive.evidence` module for deterministic SemanticChunk construction.
  - EvidencePath construction and validation over Paper -> PageIndexNode -> SemanticChunk links.
  - Vertical fixtures and tests proving full-text ingestion -> PageIndex -> chunk -> evidence path traceability.
requires:
  []
affects:
  - S04
  - S05
  - S07
  - S09
key_files:
  - src/arxiv_archive/evidence.py
  - tests/test_evidence_paths.py
key_decisions:
  - Use section-level deterministic chunking strategy `section_text_v1` before embeddings or LLMs.
  - Use `SemanticChunk.id = {page_index_node_id}:chunk-0001` for stable attachment to PageIndexNode records.
  - Use diagnostic-list based `validate_evidence_path()` for missing nodes, missing chunks, paper mismatches, node/chunk mismatches, and node path mismatches.
patterns_established:
  - Keep evidence substrate deterministic and local before introducing embeddings, extraction, graph persistence, DSPy, or RLM.
  - Validation for expected invalid scientific KG references should return data diagnostics, not exceptions.
observability_surfaces:
  - `SemanticChunk.validation_warnings` records chunk-local diagnostics when needed.
  - `PageIndexDocument.validation_warnings` records empty-section chunk omissions.
  - `SemanticChunk.provenance` records paper id, PageIndex node id, PageIndex path, chunking strategy, and source path.
  - `EvidencePath.validation_warnings` records explicit broken-link diagnostics.
  - `validate_evidence_path()` returns machine-readable string diagnostics without raising for expected invalid references.
drill_down_paths:
  - .gsd/milestones/M003-km5fty/slices/S03/tasks/T01-SUMMARY.md
  - .gsd/milestones/M003-km5fty/slices/S03/tasks/T02-SUMMARY.md
  - .gsd/milestones/M003-km5fty/slices/S03/tasks/T03-SUMMARY.md
  - .gsd/milestones/M003-km5fty/slices/S03/tasks/T04-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-17T17:30:22.860Z
blocker_discovered: false
---

# S03: Semantic chunks and evidence paths

**S03 established deterministic SemanticChunk records and EvidencePath validation over PageIndex documents.**

## What Happened

S03 delivered the evidence substrate between PageIndex and scientific extraction/storage work. It began with red vertical contract tests over S01 ingestion and S02 PageIndex construction, then implemented deterministic SemanticChunk records and EvidencePath construction/validation in `src/arxiv_archive/evidence.py`. Chunks attach to non-empty PageIndex nodes with stable ids, order, char spans, chunking strategy, PageIndex paths, source path, and provenance. EvidencePath validation now catches paper mismatches, missing PageIndex nodes, missing chunks, node/chunk mismatches, and node path mismatches while allowing fallback full-text sections as valid evidence targets. Final verification proved evidence behavior, PageIndex behavior, S01 ingestion behavior, analysis regressions, and CLI contract behavior all remain green.

## Verification

Fresh T04 verification passed: `uv run pytest tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q` reported 44 passed; Ruff reported all checks passed; public module help smoke passed; LSP diagnostics reported no diagnostics.

## Requirements Advanced

None.

## Requirements Validated

None.

## New Requirements Surfaced

- M003 requirements R026-R035 need restoration or clarification now that S01-S03 evidence substrate behavior is concrete.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

T02 implemented the initial EvidencePath boundary earlier than originally assigned because the T01 red contract covered basic EvidencePath construction. T03 then strengthened validation coverage. No scope was added beyond S03.

## Known Limitations

Chunking is intentionally simple section-level chunking. S03 does not implement claims/entities, relation extraction, embeddings, retrieval, LadybugDB persistence, DSPy, or RLM behavior. It also does not split long sections into multiple chunks yet.

## Follow-ups

Restore or clarify M003 requirements R026-R035 after S03 so traceability reflects the completed full-text, PageIndex, SemanticChunk, and EvidencePath substrate. S04 should build claim/entity/relation drafts on top of EvidencePath without revalidating PageIndex internals. Run `gitnexus analyze . --name daily-archive` before relying on graph impact for new evidence symbols.

## Files Created/Modified

- `src/arxiv_archive/evidence.py` — New deterministic SemanticChunk and EvidencePath module with chunk construction, evidence path construction, and validation diagnostics.
- `tests/test_evidence_paths.py` — New vertical contract and regression tests over S01 ingestion, S02 PageIndex, SemanticChunk construction, and EvidencePath validation.
