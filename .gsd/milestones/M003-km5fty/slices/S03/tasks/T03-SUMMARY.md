---
id: T03
parent: S03
milestone: M003-km5fty
key_files:
  - tests/test_evidence_paths.py
  - src/arxiv_archive/evidence.py
key_decisions:
  - Keep EvidencePath validation pure and fixture-local; callers pass the document and chunk list explicitly.
  - Treat fallback full-text sections as valid evidence targets when PageIndex fallback diagnostics are present.
duration: 
verification_result: passed
completed_at: 2026-05-17T17:28:26.763Z
blocker_discovered: false
---

# T03: Added EvidencePath validation coverage for missing chunks, node-path mismatch, and fallback evidence.

**Added EvidencePath validation coverage for missing chunks, node-path mismatch, and fallback evidence.**

## What Happened

Expanded `tests/test_evidence_paths.py` with validation coverage for missing SemanticChunk references, mismatched node paths, and valid fallback-section evidence paths. Existing `validate_evidence_path()` behavior already satisfied the new cases, so no production code changes were required. This confirms S04 can reference evidence paths without revalidating PageIndex internals and can inspect explicit diagnostics for broken links.

## Verification

Ran `uv run pytest tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py -q`; 21 tests passed. Ran `uv run ruff check src/arxiv_archive/evidence.py tests/test_evidence_paths.py`; all checks passed. LSP diagnostics for `tests/test_evidence_paths.py` reported no diagnostics. GitNexus change detection reported low risk with no indexed changed symbols.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py -q` | 0 | ✅ pass: 21 tests passed | 150ms |
| 2 | `uv run ruff check src/arxiv_archive/evidence.py tests/test_evidence_paths.py` | 0 | ✅ pass: Ruff clean | 0ms |
| 3 | `lsp diagnostics tests/test_evidence_paths.py` | 0 | ✅ pass: no diagnostics | 0ms |

## Deviations

No production changes were needed because T02 had already implemented the initial EvidencePath boundary required by the red contract. T03 added the planned missing chunk, node path mismatch, and fallback evidence-path validation coverage.

## Known Issues

GitNexus has not indexed new evidence symbols yet; impact calls return target-not-found until `gitnexus analyze . --name daily-archive` is rerun.

## Files Created/Modified

- `tests/test_evidence_paths.py`
- `src/arxiv_archive/evidence.py`
