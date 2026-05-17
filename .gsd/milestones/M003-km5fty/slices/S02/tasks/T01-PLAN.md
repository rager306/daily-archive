---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Add PageIndex contract tests and fallback fixture

Create red contract tests for PageIndex construction over the S01 structured markdown fixture and a malformed/no-heading fallback fixture. Define expected `PageIndexNode` fields, deterministic IDs, parent/child relationships, ordered NEXT traversal, stable Paper -> node paths, and validation diagnostics. Done when the new tests fail for missing `arxiv_archive.page_index` while S01 ingestion tests still pass.

## Inputs

- `.gsd/milestones/M003-km5fty/slices/S01/S01-SUMMARY.md`
- `tests/fixtures/full_text/structured_paper.md`
- `src/arxiv_archive/full_text.py`

## Expected Output

- `tests/fixtures/page_index/no_headings.txt`
- `tests/test_page_index.py`

## Verification

uv run pytest tests/test_page_index.py -q

## Observability Impact

Defines diagnostic expectations for structure parsing failures, fallback sections, and ordered navigation errors before implementation.
