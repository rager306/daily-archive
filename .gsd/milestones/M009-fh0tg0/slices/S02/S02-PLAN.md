# S02: S02

**Goal:** Expose artifact freshness verification through validation-batch CLI so recorded provenance logs can be checked from the command line.
**Demo:** After this slice, a verifier can prove whether an artifact set matches a recorded CLI run or fail on stale/mismatched outputs.

## Must-Haves

- `validation-batch verify-artifacts` reads a provenance JSONL log and selects a run by run-id or batch/command.
- CLI exits 0 only for `fresh` verdict and nonzero for stale/missing/invalid provenance.
- CLI can write a freshness report artifact.
- Tests prove mutated outputs and missing outputs fail.
- Existing validation-batch CLI tests remain compatible.

## Proof Level

- This slice proves: CLI tests for fresh pass, stale failure, missing failure, report writing, and redaction.

## Integration Closure

Consumes S01 provenance/freshness primitives and provides a CLI gate for future provenance-integrated validation-batch runs.

## Verification

- Adds verifier reports with explicit fresh/stale/missing/invalid provenance verdicts and diagnostics.

## Tasks

- [x] **T01: Added the additive `validation-batch verify-artifacts` CLI command.** `est:medium`
  Add `validation-batch verify-artifacts` CLI command using S01 provenance helpers. The command should accept provenance log, optional run-id, batch-id, command, optional report path, and --json; return exit 0 only on fresh verdict.
  - Files: `src/arxiv_archive/cli.py`
  - Verify: uv run python -m arxiv_archive validation-batch --help | grep -Fq 'verify-artifacts'

- [x] **T02: Added freshness verifier CLI tests for fresh, stale, missing, input-mutation, and redaction cases.** `est:medium`
  Add CLI tests for verify-artifacts fresh pass, report writing, stale mutation failure, missing output failure, input mutation failure, and redaction of raw fixture content.
  - Files: `tests/test_validation_batch_cli_freshness.py`
  - Verify: uv run pytest tests/test_validation_batch_cli_freshness.py -q && uv run ruff check src/arxiv_archive/cli.py tests/test_validation_batch_cli_freshness.py

- [x] **T03: Generated freshness verifier pass/fail sample reports and ran focused regression.** `est:small`
  Generate S02 sample freshness reports for a fresh and stale artifact set, then run focused CLI regression tests.
  - Files: `.gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-pass-report.json`, `.gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-stale-report.json`
  - Verify: uv run pytest tests/test_validation_batch_provenance.py tests/test_validation_batch_cli_freshness.py tests/test_validation_batch_cli_contract.py -q && uv run ruff check src/arxiv_archive/cli.py src/arxiv_archive/validation_batch_provenance.py tests/test_validation_batch_cli_freshness.py && test -s .gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-pass-report.json && test -s .gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-stale-report.json

## Files Likely Touched

- src/arxiv_archive/cli.py
- tests/test_validation_batch_cli_freshness.py
- .gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-pass-report.json
- .gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-stale-report.json
