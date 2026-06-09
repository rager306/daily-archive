---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Added the additive `validation-batch verify-artifacts` CLI command.

Add `validation-batch verify-artifacts` CLI command using S01 provenance helpers. The command should accept provenance log, optional run-id, batch-id, command, optional report path, and --json; return exit 0 only on fresh verdict.

## Inputs

- `src/arxiv_archive/validation_batch_provenance.py`
- `src/arxiv_archive/cli.py`

## Expected Output

- `src/arxiv_archive/cli.py`

## Verification

uv run python -m arxiv_archive validation-batch --help | grep -Fq 'verify-artifacts'

## Observability Impact

Makes freshness verification directly runnable and automatable.
