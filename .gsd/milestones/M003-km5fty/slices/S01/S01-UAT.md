# S01: Full text ingestion contract — UAT

**Milestone:** M003-km5fty
**Written:** 2026-05-17T16:31:44.491Z

# UAT — S01 Full text ingestion contract

## Acceptance checks

1. Structured markdown fixture ingests with `extraction_mode == "structured_markdown"`, no warnings, no fallback reason, and provenance containing paper id/source/source path/extraction mode.
2. Plain text fixture ingests with explicit `plain_text` fallback diagnostics.
3. Missing and empty sources return typed diagnostic results rather than silent bad output.
4. Stored paper id plus deterministic `papers/{paper_id}/full_text.md` path produces a PageIndex-ready ingestion result.
5. Existing analysis and CLI contract behavior remains unchanged.

## Evidence

- `uv run pytest tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q` → 29 passed.
- `uv run ruff check src/arxiv_archive/full_text.py tests/test_full_text_ingestion.py src/arxiv_archive/cli.py tests/test_cli_contract.py` → All checks passed.
- `uv run python -m arxiv_archive --help` with usage/date/json/cron/Hermes/status lifecycle token assertions → passed.
- LSP diagnostics for `src/arxiv_archive/full_text.py` → no diagnostics.

