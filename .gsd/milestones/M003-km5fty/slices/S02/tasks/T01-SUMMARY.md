---
id: T01
parent: S02
milestone: M003-km5fty
key_files:
  - tests/fixtures/page_index/no_headings.txt
  - tests/test_page_index.py
key_decisions:
  - Define the PageIndex construction entrypoint as `build_page_index(ingestion)`.
  - Use deterministic node IDs derived from paper id and slugged section titles, with root id `{paper_id}:root`.
  - Require validation diagnostics for no-heading fallback input before implementation.
duration: 
verification_result: passed
completed_at: 2026-05-17T16:37:29.453Z
blocker_discovered: false
---

# T01: Added red PageIndex contract tests and a fallback no-heading fixture for S02.

**Added red PageIndex contract tests and a fallback no-heading fixture for S02.**

## What Happened

Created the S02 no-heading fallback fixture and a new `tests/test_page_index.py` contract suite. The tests define the future PageIndex contract over S01 ingestion results: ordered PageIndexNode tree construction from structured markdown, deterministic root and section IDs, parent/child links, NEXT traversal, stable Paper -> PageIndexNode paths, case-insensitive section lookup, and explicit fallback diagnostics for no-heading text. The focused test command fails during collection because `arxiv_archive.page_index` does not exist yet, which is the intended T02 implementation boundary.

## Verification

Ran `uv run pytest tests/test_page_index.py -q`; it failed as expected during collection with `ModuleNotFoundError: No module named 'arxiv_archive.page_index'`. Ran `uv run pytest tests/test_full_text_ingestion.py -q`; 6 tests passed. Ran `uv run ruff check tests/test_page_index.py`; all checks passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_page_index.py -q` | 2 | ✅ expected red contract: collection fails on missing arxiv_archive.page_index implementation boundary | 170ms |
| 2 | `uv run pytest tests/test_full_text_ingestion.py -q` | 0 | ✅ pass: 6 S01 ingestion tests passed | 120ms |
| 3 | `uv run ruff check tests/test_page_index.py` | 0 | ✅ pass: new PageIndex test file lint clean | 0ms |

## Deviations

None; T01 intentionally ends red because S02 T02 owns implementation.

## Known Issues

`arxiv_archive.page_index` is not implemented yet; T02 must add `build_page_index`, PageIndex document/node models, navigation helpers, and validation diagnostics matching the red contract.

## Files Created/Modified

- `tests/fixtures/page_index/no_headings.txt`
- `tests/test_page_index.py`
