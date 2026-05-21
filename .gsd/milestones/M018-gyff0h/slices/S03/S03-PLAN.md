# S03: Dependency security triage recommendation

**Goal:** Produce final triage: update, remove, isolate, or defer each vulnerable ML package with rationale and verification.
**Demo:** After S03, the project has an actionable dependency security recommendation and R046 is validated or explicitly blocked.

## Must-Haves

- Findings are severity-ranked by reachability and exploitability.
- Recommendation is explicit for torch/transformers.
- R046 is updated with validation or blocker evidence.
- No broad dependency upgrade is performed without a separate milestone.

## Proof Level

- This slice proves: Review report plus final guard assertions and independent security review where feasible.

## Integration Closure

Closes M018 with a recommendation for follow-up remediation if needed.

## Verification

- Final guard captures active runtime exposure and urgency.

## Tasks

- [x] **T01: Write final dependency security triage** `est:60m`
  Synthesize S01/S02 into a final dependency security triage report. Recommend whether to update, remove, isolate, or defer torch/transformers and Docling fallback. Include severity, exploitability, affected path, and follow-up milestone recommendation.
  - Files: `.gsd/milestones/M018-gyff0h/slices/S03/dependency-security-triage.md`, `.gsd/milestones/M018-gyff0h/slices/S03/run-evidence/final-dependency-security-guard.json`
  - Verify: uv run python inline assertions over final-dependency-security-guard.json

- [x] **T02: Review and validate R046** `est:45m`
  Run independent security review of the final triage artifacts and update R046 with validation evidence. Close M018 if the review agrees no immediate hotfix is required.
  - Files: `.gsd/milestones/M018-gyff0h/slices/S03/run-evidence/independent-security-review.md`, `.gsd/REQUIREMENTS.md`
  - Verify: uv run python inline assertions over review and guard artifacts

## Files Likely Touched

- .gsd/milestones/M018-gyff0h/slices/S03/dependency-security-triage.md
- .gsd/milestones/M018-gyff0h/slices/S03/run-evidence/final-dependency-security-guard.json
- .gsd/milestones/M018-gyff0h/slices/S03/run-evidence/independent-security-review.md
- .gsd/REQUIREMENTS.md
