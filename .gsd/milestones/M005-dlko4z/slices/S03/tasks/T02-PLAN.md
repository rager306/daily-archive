---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Parse markdown structure with canonical spans

Implement deterministic parsing from canonical normalized Markdown into structural elements with absolute character spans and parent-child hierarchy. Cover headings, paragraphs, references, tables, figures/captions, and equation-like blocks where detectable without LLMs.

## Inputs

- `tests/fixtures/full_text/structured_paper.md`
- `tests/fixtures/full_text/arxiv_landing_only.md`

## Expected Output

- `src/arxiv_archive/structure_aware_chunking.py`
- `tests/test_structure_aware_chunking.py`

## Verification

uv run pytest tests/test_structure_aware_chunking.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py

## Observability Impact

Diagnostics should expose source-span coverage and hierarchy coverage without logging raw text.
