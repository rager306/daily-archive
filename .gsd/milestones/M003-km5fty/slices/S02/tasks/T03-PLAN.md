---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Added PageIndex navigation lookup helpers and structural validation diagnostics.

Add navigation helpers and tests for locating sections by title, computing stable Paper -> PageIndexNode paths, walking NEXT links, and validating parent/child/NEXT consistency. Keep helpers pure and fixture-local. Done when downstream S03 can attach chunks to stable node IDs without inspecting parser internals.

## Inputs

- `src/arxiv_archive/page_index.py`
- `tests/test_page_index.py`

## Expected Output

- `src/arxiv_archive/page_index.py`
- `tests/test_page_index.py`

## Verification

uv run pytest tests/test_page_index.py tests/test_full_text_ingestion.py -q

## Observability Impact

Validation diagnostics identify broken path, parent, order, or NEXT-link invariants explicitly.
