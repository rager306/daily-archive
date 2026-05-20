---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T02: Test freshness verifier CLI

Add CLI tests for verify-artifacts fresh pass, report writing, stale mutation failure, missing output failure, input mutation failure, and redaction of raw fixture content.

## Inputs

- `src/arxiv_archive/cli.py`
- `src/arxiv_archive/validation_batch_provenance.py`
- `tests/test_validation_batch_provenance.py`

## Expected Output

- `tests/test_validation_batch_cli_freshness.py`

## Verification

uv run pytest tests/test_validation_batch_cli_freshness.py -q && uv run ruff check src/arxiv_archive/cli.py tests/test_validation_batch_cli_freshness.py

## Observability Impact

Negative tests prove the verifier catches fake/stale artifacts instead of trusting summaries.
