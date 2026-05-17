---
id: T01
parent: S03
milestone: M003-km5fty
key_files:
  - tests/test_evidence_paths.py
key_decisions:
  - Define the S03 module boundary as `arxiv_archive.evidence`.
  - Use `build_semantic_chunks(document)` and `build_evidence_path(document, chunk)` as pure construction entrypoints.
  - Keep EvidencePath validation deterministic and diagnostic-list based.
duration: 
verification_result: passed
completed_at: 2026-05-17T17:24:23.157Z
blocker_discovered: false
---

# T01: Added red SemanticChunk and EvidencePath contract tests for S03.

**Added red SemanticChunk and EvidencePath contract tests for S03.**

## What Happened

Added `tests/test_evidence_paths.py` as a red vertical contract over S01 ingestion and S02 PageIndex construction. The tests define deterministic `SemanticChunk` IDs, PageIndexNode attachment, section/fallback chunk behavior, chunk provenance and spans, EvidencePath construction, and validation diagnostics for missing or mismatched links. The focused test command fails during collection because `arxiv_archive.evidence` does not exist yet, which is the intended S03 T02/T03 implementation boundary.

## Verification

Ran `uv run pytest tests/test_evidence_paths.py -q`; it failed as expected during collection with `ModuleNotFoundError: No module named 'arxiv_archive.evidence'`. Ran `uv run pytest tests/test_page_index.py -q`; 6 tests passed. Ran `uv run ruff check tests/test_evidence_paths.py`; all checks passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_evidence_paths.py -q` | 2 | ✅ expected red contract: collection fails on missing arxiv_archive.evidence implementation boundary | 180ms |
| 2 | `uv run pytest tests/test_page_index.py -q` | 0 | ✅ pass: 6 PageIndex tests passed | 140ms |
| 3 | `uv run ruff check tests/test_evidence_paths.py` | 0 | ✅ pass: new evidence-path test file lint clean | 0ms |

## Deviations

None; T01 intentionally ends red because S03 T02 owns implementation.

## Known Issues

`arxiv_archive.evidence` is not implemented yet; T02 must add `SemanticChunk`, chunk construction, and T03 must add `EvidencePath` validation matching this contract.

## Files Created/Modified

- `tests/test_evidence_paths.py`
