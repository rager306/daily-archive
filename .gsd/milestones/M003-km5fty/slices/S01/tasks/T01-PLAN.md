---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T01: Added S01 full-text ingestion contract tests and deterministic fixtures.

Create deterministic fixture inputs and red contract tests for the S01 full-text ingestion boundary before implementation. Add representative markdown/full-text fixtures under tests, define expected behavior for structured markdown, plain-text fallback, missing source, and malformed or empty content. Done when the new tests fail for missing implementation while existing test collection still runs.

## Inputs

- `.gsd/PROJECT.md`
- `.gsd/REQUIREMENTS.md`
- `src/arxiv_archive/cli.py`
- `src/arxiv_archive/ladybug_client.py`

## Expected Output

- `tests/fixtures/full_text/structured_paper.md`
- `tests/fixtures/full_text/plain_fallback.txt`
- `tests/test_full_text_ingestion.py`

## Verification

uv run pytest tests/test_full_text_ingestion.py -q

## Observability Impact

The tests define the required diagnostic fields: source path, source type, extraction mode, warnings, fallback reason, and provenance.
