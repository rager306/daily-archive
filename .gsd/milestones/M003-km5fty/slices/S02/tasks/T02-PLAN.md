---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Implemented PageIndex document construction from S01 full-text ingestion results.

Implement `src/arxiv_archive/page_index.py` with typed `PageIndexNode`, `PageIndexDocument`, and construction from `FullTextIngestionResult`. Parse markdown headings into a hierarchy with deterministic node IDs and ordered relationships; create an explicit fallback root/section when no headings exist. Done when the initial PageIndex contract tests pass.

## Inputs

- `tests/test_page_index.py`
- `src/arxiv_archive/full_text.py`

## Expected Output

- `src/arxiv_archive/page_index.py`

## Verification

uv run pytest tests/test_page_index.py tests/test_full_text_ingestion.py -q

## Observability Impact

Construction result exposes validation diagnostics and parser mode in code-readable fields.
