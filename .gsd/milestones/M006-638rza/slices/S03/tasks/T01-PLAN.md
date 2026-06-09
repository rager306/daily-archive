---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Implemented the 30-paper deviation scanner and tests.

Implement a deterministic 30-paper deviation analysis helper that consumes the M006 manifest and available Markdown sources, reuses structure-aware chunking/package diagnostics where possible, and writes redacted per-paper metrics. The helper must not serialize raw Markdown/chunk text.

## Inputs

- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json`
- `.gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-summary.json`
- `src/arxiv_archive/structure_aware_chunking.py`
- `src/arxiv_archive/chunking_benchmark.py`

## Expected Output

- `src/arxiv_archive/thirty_paper_deviation_scan.py`
- `tests/test_thirty_paper_deviation_scan.py`

## Verification

uv run pytest tests/test_thirty_paper_deviation_scan.py tests/test_structure_aware_chunking.py tests/test_chunking_benchmark.py -q && uv run ruff check src/arxiv_archive/thirty_paper_deviation_scan.py tests/test_thirty_paper_deviation_scan.py

## Observability Impact

Helper emits per-paper metrics and aggregate distributions that enable outlier detection without raw content exposure.
