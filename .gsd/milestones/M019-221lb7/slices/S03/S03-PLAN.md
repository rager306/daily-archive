# S03: S03

**Goal:** Synthesize profiles into a comparative matrix and next-step recommendation for Scientific KG work.
**Demo:** After S03, daily-archive has an actionable recommendation: which patterns to reuse, which to reject, and how this affects the KG candidate-locator roadmap.

## Must-Haves

- Comparative matrix covers all four systems.
- Recommendation names concrete reusable patterns and rejected patterns.
- No production activation is proposed without a future milestone.
- R047 is validated or explicitly blocked with evidence.

## Proof Level

- This slice proves: Comparison matrix, final guard, and independent review.

## Integration Closure

Closes spike and points to candidate locators/chunk-span provenance or targeted follow-up.

## Verification

- Final matrix and recommendation become durable planning evidence.

## Tasks

- [x] **T01: Wrote final research-agent comparative matrix and recommendation.** `est:60m`
  Synthesize the four S02 profiles into a comparative matrix covering architecture, source acquisition, provenance, review gates, autonomy, failure modes, reusable patterns, and non-goals. Write final recommendation for daily-archive's next KG/provenance milestone.
  - Files: `.gsd/milestones/M019-221lb7/slices/S03/research-agent-comparative-matrix.md`, `.gsd/milestones/M019-221lb7/slices/S03/run-evidence/final-research-agent-spike-guard.json`
  - Verify: uv run python inline assertions over final guard and matrix

- [x] **T02: Validated R047 after independent recommendation review passed.** `est:45m`
  Run independent review of the comparative recommendation, update R047, and close M019 if the review agrees the spike is evidence-backed and no unsafe adoption is proposed.
  - Files: `.gsd/milestones/M019-221lb7/slices/S03/run-evidence/independent-recommendation-review.md`, `.gsd/REQUIREMENTS.md`
  - Verify: uv run python inline assertions over review, guard, and R047 evidence

## Files Likely Touched

- .gsd/milestones/M019-221lb7/slices/S03/research-agent-comparative-matrix.md
- .gsd/milestones/M019-221lb7/slices/S03/run-evidence/final-research-agent-spike-guard.json
- .gsd/milestones/M019-221lb7/slices/S03/run-evidence/independent-recommendation-review.md
- .gsd/REQUIREMENTS.md
