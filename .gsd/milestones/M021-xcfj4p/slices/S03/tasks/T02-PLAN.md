---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Run deterministic bounded batch rehearsal

Run the deterministic batch helper on the M011 bounded targets and persist M021 S03 run evidence with richer ambiguity diagnostics than M020.

## Inputs

- `src/arxiv_archive/candidate_locators.py`
- `.gsd/milestones/M011-2f8j8m/slices/S01/run-evidence/semantic-review-targets.json`

## Expected Output

- `.gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch.json`
- `.gsd/milestones/M021-xcfj4p/slices/S03/deterministic-locator-batch-report.md`

## Verification

uv run python inline batch generation and assertions

## Observability Impact

Produces batch output and explanatory ambiguity metrics for S04.
