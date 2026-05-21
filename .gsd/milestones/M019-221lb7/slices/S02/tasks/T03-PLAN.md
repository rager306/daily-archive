---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T03: Write profile index

Run a profile completeness guard over all four profile artifacts and write an S02 profile index report summarizing profile confidence and known gaps.

## Inputs

- `.gsd/milestones/M019-221lb7/slices/S02/profiles/gpt-researcher-profile.md`
- `.gsd/milestones/M019-221lb7/slices/S02/profiles/ai-researcher-profile.md`
- `.gsd/milestones/M019-221lb7/slices/S02/profiles/ai-scientist-profile.md`
- `.gsd/milestones/M019-221lb7/slices/S02/profiles/prismaid-profile.md`

## Expected Output

- `.gsd/milestones/M019-221lb7/slices/S02/research-agent-profile-index.md`

## Verification

uv run python inline assertions over profile files and index

## Observability Impact

Creates profile inventory for S03 synthesis.
