# S01: Full text ingestion contract

**Goal:** Define the full-text and markdown ingestion contract required for PageIndex and scientific evidence paths.
**Demo:** After this: a representative paper fixture has stable full-text or markdown input, deterministic IDs, provenance, and parser fallbacks ready for PageIndex construction.

## Must-Haves

- Full-text/markdown fixture inputs are stored under deterministic test paths.
- Ingestion contract records paper id, source type, source path, text, provenance, and warnings.
- Poor or missing structure produces explicit fallback metadata rather than silent bad output.
- Normal tests do not require live PDF/network access.

## Proof Level

- This slice proves: fixture and contract tests

## Integration Closure

Connects existing paper artifacts to the future PageIndex and chunk pipeline without changing the daily cron contract. S01 produces a deterministic ingestion boundary consumed by S02 PageIndex construction and later SemanticChunk/EvidencePath work.

## Verification

- Records source path, extraction mode, parser warnings, fallback reason, and provenance for each full-text ingestion attempt. Failure states are represented in typed ingestion results rather than hidden behind silent empty text.

## Tasks

- [x] **T01: Add full text ingestion contract tests and fixtures** `est:45m`
  Create deterministic fixture inputs and red contract tests for the S01 full-text ingestion boundary before implementation. Add representative markdown/full-text fixtures under tests, define expected behavior for structured markdown, plain-text fallback, missing source, and malformed or empty content. Done when the new tests fail for missing implementation while existing test collection still runs.
  - Files: `tests/fixtures/full_text/structured_paper.md`, `tests/fixtures/full_text/plain_fallback.txt`, `tests/test_full_text_ingestion.py`
  - Verify: uv run pytest tests/test_full_text_ingestion.py -q

- [x] **T02: Implement local full text ingestion boundary** `est:1h`
  Implement the minimal production ingestion boundary for local markdown and plain text sources. Add typed dataclasses or equivalent models for `FullTextSource`, `FullTextIngestionResult`, provenance, warnings, and fallback metadata. The implementation must be deterministic, local-only, and must not fetch PDFs or use network services. Done when S01 contract tests pass for structured markdown, plain text fallback, missing files, and empty/malformed content.
  - Files: `src/arxiv_archive/full_text.py`, `tests/test_full_text_ingestion.py`
  - Verify: uv run pytest tests/test_full_text_ingestion.py -q

- [ ] **T03: Verify artifact to ingestion boundary for PageIndex consumers** `est:45m`
  Wire the ingestion boundary to existing stored paper artifact assumptions without changing the public daily CLI. Add tests proving a stored paper id plus deterministic local source path can produce an ingestion result ready for PageIndex construction, and document the S01 boundary in module docstrings or test names. Run targeted tests plus lint on the new production module. Done when future S02 can consume the result shape without touching M001 cron artifacts.
  - Files: `src/arxiv_archive/full_text.py`, `tests/test_full_text_ingestion.py`
  - Verify: uv run pytest tests/test_full_text_ingestion.py tests/test_analysis.py -q

- [ ] **T04: Run S01 quality gates and regression smoke** `est:30m`
  Run final S01 quality gates and record any known limitations for downstream S02. Execute targeted ingestion tests, relevant regression tests, Ruff on touched files, and public CLI help smoke to ensure the full-text contract did not alter M001/M002 public surfaces. Done when all commands pass and the slice is ready for execution closeout.
  - Files: `src/arxiv_archive/full_text.py`, `tests/test_full_text_ingestion.py`
  - Verify: uv run pytest tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q

## Files Likely Touched

- tests/fixtures/full_text/structured_paper.md
- tests/fixtures/full_text/plain_fallback.txt
- tests/test_full_text_ingestion.py
- src/arxiv_archive/full_text.py
