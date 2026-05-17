---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Implement local full text ingestion boundary

Implement the minimal production ingestion boundary for local markdown and plain text sources. Add typed dataclasses or equivalent models for `FullTextSource`, `FullTextIngestionResult`, provenance, warnings, and fallback metadata. The implementation must be deterministic, local-only, and must not fetch PDFs or use network services. Done when S01 contract tests pass for structured markdown, plain text fallback, missing files, and empty/malformed content.

## Inputs

- `tests/test_full_text_ingestion.py`
- `tests/fixtures/full_text/structured_paper.md`
- `tests/fixtures/full_text/plain_fallback.txt`

## Expected Output

- `src/arxiv_archive/full_text.py`
- `tests/test_full_text_ingestion.py`

## Verification

uv run pytest tests/test_full_text_ingestion.py -q

## Observability Impact

The ingestion result exposes warnings and fallback metadata in code-readable fields so future agents can diagnose parser quality without inspecting logs.
