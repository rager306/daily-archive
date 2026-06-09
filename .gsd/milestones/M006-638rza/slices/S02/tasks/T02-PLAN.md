---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Implemented the bounded source acquisition helper and tests.

Implement a reusable 30-paper source acquisition/audit helper or script that attempts Markdown/PDF acquisition for missing papers using bounded project mechanisms. It must write per-paper redacted diagnostics and avoid raw text in JSON/JSONL.

## Inputs

- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json`
- `src/arxiv_archive/full_text.py`
- `src/arxiv_archive/md_converter.py`
- `src/arxiv_archive/pdf_downloader.py`

## Expected Output

- `src/arxiv_archive/thirty_paper_source_scan.py`
- `tests/test_thirty_paper_source_scan.py`

## Verification

uv run pytest tests/test_thirty_paper_source_scan.py -q && uv run ruff check src/arxiv_archive/thirty_paper_source_scan.py tests/test_thirty_paper_source_scan.py

## Observability Impact

Adds structured diagnostics for acquisition/conversion attempts and updated availability state.
