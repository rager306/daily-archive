---
id: T02
parent: S03
milestone: M003-km5fty
key_files:
  - src/arxiv_archive/evidence.py
  - tests/test_evidence_paths.py
key_decisions:
  - Use section-level deterministic chunking strategy `section_text_v1` before embeddings or LLMs.
  - Skip the PageIndex root node and emit chunks only for non-empty child/fallback nodes.
  - Keep chunk and evidence validation diagnostics as returned lists of strings.
duration: 
verification_result: passed
completed_at: 2026-05-17T17:26:56.727Z
blocker_discovered: false
---

# T02: Implemented deterministic SemanticChunk construction and the initial EvidencePath boundary.

**Implemented deterministic SemanticChunk construction and the initial EvidencePath boundary.**

## What Happened

Implemented `src/arxiv_archive/evidence.py` with `SemanticChunk`, `EvidencePath`, `build_semantic_chunks()`, `build_evidence_path()`, and `validate_evidence_path()`. Chunking is deterministic and local-only: it emits section-level chunks for non-empty non-root PageIndex nodes, preserves chunk order, stable IDs, character spans, source node ids, PageIndex paths, chunking strategy, and provenance. Evidence-path construction and validation now prove the Paper -> PageIndexNode -> SemanticChunk references are internally consistent and produce explicit diagnostics when links mismatch.

## Verification

Ran `uv run pytest tests/test_evidence_paths.py tests/test_page_index.py -q`; 12 tests passed. Ran `uv run ruff check src/arxiv_archive/evidence.py tests/test_evidence_paths.py`; all checks passed. LSP diagnostics for `src/arxiv_archive/evidence.py` and `tests/test_evidence_paths.py` reported no diagnostics. GitNexus change detection reported low risk with no indexed changed symbols.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_evidence_paths.py tests/test_page_index.py -q` | 0 | ✅ pass: 12 tests passed | 150ms |
| 2 | `uv run ruff check src/arxiv_archive/evidence.py tests/test_evidence_paths.py` | 0 | ✅ pass: Ruff clean | 0ms |
| 3 | `lsp diagnostics src/arxiv_archive/evidence.py and tests/test_evidence_paths.py` | 0 | ✅ pass: no diagnostics | 0ms |

## Deviations

T02 also implemented the minimal EvidencePath dataclass/build/validation boundary because the red T01 contract included basic evidence-path behavior. T03 will strengthen validation coverage rather than introduce the boundary from scratch.

## Known Issues

GitNexus has not indexed new evidence symbols yet; impact calls return target-not-found until `gitnexus analyze . --name daily-archive` is rerun.

## Files Created/Modified

- `src/arxiv_archive/evidence.py`
- `tests/test_evidence_paths.py`
