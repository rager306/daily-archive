# S03: Active scan lineage metadata

**Goal:** Fix validation-batch scan lineage metadata so scan artifacts carry the active milestone/batch context and provenance verification can catch lineage mismatches.
**Demo:** After this slice, validation scan summaries identify M009/M008-style active milestone and batch context instead of stale M006 metadata.

## Must-Haves

- Validation-batch scan artifacts include active `milestone_id` and `batch_id` metadata.
- Reused scanner no longer leaves hardcoded M006 milestone metadata as the authoritative validation-batch lineage.
- Freshness verifier can fail when expected lineage metadata does not match artifact contents.
- Existing CLI scan behavior remains compatible.
- Safety flags remain false and no import/write behavior is added.

## Proof Level

- This slice proves: Focused scan tests, verifier mismatch tests, sample scan artifacts, and ruff.

## Integration Closure

Consumes S01/S02 provenance verifier primitives and updates validation-batch scan artifact production for future batches.

## Verification

- Adds milestone_id/batch_id lineage fields to scan summary, delta, outlier, and freshness verification diagnostics.

## Tasks

- [x] **T01: Add active scan lineage metadata** `est:medium`
  Add active lineage metadata support to validation-batch scan artifact production. Keep compatibility with existing scanner outputs, but ensure validation-batch summary/delta/outlier artifacts expose active milestone_id and batch_id.
  - Files: `src/arxiv_archive/validation_batch_workflow.py`
  - Verify: uv run pytest tests/test_validation_batch_scan_workflow.py tests/test_validation_batch_cli_scan.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py

- [x] **T02: Verify artifact lineage metadata** `est:medium`
  Extend provenance freshness verification to optionally check expected artifact metadata fields, then add tests proving mismatched milestone/batch metadata fails.
  - Files: `src/arxiv_archive/validation_batch_provenance.py`, `tests/test_validation_batch_provenance.py`
  - Verify: uv run pytest tests/test_validation_batch_provenance.py -q && uv run ruff check src/arxiv_archive/validation_batch_provenance.py tests/test_validation_batch_provenance.py

- [x] **T03: Run lineage regression and sample evidence** `est:small`
  Generate S03 sample scan/freshness evidence showing active M009 lineage and a negative lineage mismatch report.
  - Files: `.gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-pass-report.json`, `.gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-mismatch-report.json`
  - Verify: uv run pytest tests/test_validation_batch_provenance.py tests/test_validation_batch_scan_workflow.py tests/test_validation_batch_cli_scan.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py src/arxiv_archive/validation_batch_provenance.py tests/test_validation_batch_provenance.py && test -s .gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-pass-report.json && test -s .gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-mismatch-report.json

## Files Likely Touched

- src/arxiv_archive/validation_batch_workflow.py
- src/arxiv_archive/validation_batch_provenance.py
- tests/test_validation_batch_provenance.py
- .gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-pass-report.json
- .gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-mismatch-report.json
