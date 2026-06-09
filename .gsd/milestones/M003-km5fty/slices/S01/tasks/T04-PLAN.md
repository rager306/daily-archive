---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T04: Ran final S01 regression, lint, diagnostics, and public CLI help smoke successfully.

Run final S01 quality gates and record any known limitations for downstream S02. Execute targeted ingestion tests, relevant regression tests, Ruff on touched files, and public CLI help smoke to ensure the full-text contract did not alter M001/M002 public surfaces. Done when all commands pass and the slice is ready for execution closeout.

## Inputs

- `src/arxiv_archive/full_text.py`
- `tests/test_full_text_ingestion.py`
- `src/arxiv_archive/cli.py`

## Expected Output

- `src/arxiv_archive/full_text.py`
- `tests/test_full_text_ingestion.py`

## Verification

uv run pytest tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q

## Observability Impact

Final verification checks both feature behavior and diagnostic fields so failure inspection remains agent-friendly.
