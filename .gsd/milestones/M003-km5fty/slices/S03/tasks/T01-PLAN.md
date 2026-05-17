---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Add SemanticChunk and EvidencePath contract tests

Create red contract tests for `SemanticChunk`, `EvidencePath`, deterministic chunk IDs, chunk ordering, PageIndexNode attachment, empty/fallback section behavior, and validation diagnostics. Tests should consume S01/S02 fixtures through `ingest_full_text()` and `build_page_index()` so the contract is vertical. Done when the new tests fail for missing semantic/evidence implementation while PageIndex tests still pass.

## Inputs

- `.gsd/milestones/M003-km5fty/slices/S02/S02-SUMMARY.md`
- `src/arxiv_archive/full_text.py`
- `src/arxiv_archive/page_index.py`
- `tests/fixtures/full_text/structured_paper.md`
- `tests/fixtures/page_index/no_headings.txt`

## Expected Output

- `tests/test_evidence_paths.py`

## Verification

uv run pytest tests/test_evidence_paths.py -q

## Observability Impact

Defines expected diagnostic fields for chunk boundaries, source node references, deterministic IDs, and evidence-path validation failures before implementation.
