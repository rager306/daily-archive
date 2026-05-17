# S02: PageIndex document navigation — UAT

**Milestone:** M003-km5fty
**Written:** 2026-05-17T16:46:39.275Z

# UAT — S02 PageIndex document navigation

## Acceptance checks

1. Structured markdown fixture produces a root and ordered section PageIndexNode tree.
2. PageIndex nodes preserve deterministic ids, title, level, order, parent, children, NEXT link, path, source path, and provenance.
3. Navigation can locate sections by title, return children, compute stable paths, and walk NEXT links in document order.
4. No-heading fallback input creates an explicit fallback full-text section and validation warning.
5. Validation diagnostics report broken parent/child/path/NEXT invariants.
6. Existing S01 ingestion, analysis, and public CLI contracts remain unchanged.

## Evidence

- `uv run pytest tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q` → 35 passed.
- `uv run ruff check src/arxiv_archive/page_index.py tests/test_page_index.py src/arxiv_archive/full_text.py tests/test_full_text_ingestion.py src/arxiv_archive/cli.py tests/test_cli_contract.py` → All checks passed.
- `uv run python -m arxiv_archive --help` with usage/date/json/cron/Hermes/status lifecycle token assertions → passed.
- LSP diagnostics for `src/arxiv_archive/page_index.py` → no diagnostics.

