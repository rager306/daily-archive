---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Write comparative matrix and recommendation

Synthesize the four S02 profiles into a comparative matrix covering architecture, source acquisition, provenance, review gates, autonomy, failure modes, reusable patterns, and non-goals. Write final recommendation for daily-archive's next KG/provenance milestone.

## Inputs

- `.gsd/milestones/M019-221lb7/slices/S02/profiles/gpt-researcher-profile.md`
- `.gsd/milestones/M019-221lb7/slices/S02/profiles/ai-researcher-profile.md`
- `.gsd/milestones/M019-221lb7/slices/S02/profiles/ai-scientist-profile.md`
- `.gsd/milestones/M019-221lb7/slices/S02/profiles/prismaid-profile.md`

## Expected Output

- `.gsd/milestones/M019-221lb7/slices/S03/research-agent-comparative-matrix.md`
- `.gsd/milestones/M019-221lb7/slices/S03/run-evidence/final-research-agent-spike-guard.json`

## Verification

uv run python inline assertions over final guard and matrix

## Observability Impact

Creates final matrix and machine-readable guard.
