# S02: Per-system architecture profiles

**Goal:** Profile each system's architecture, workflow boundaries, provenance model, autonomy level, and applicability to daily-archive.
**Demo:** After S02, each system has a concise evidence-backed profile focused on reusable patterns and risks.

## Must-Haves

- Four system profiles exist or justified gaps are documented.
- Profiles cite concrete repo/docs evidence.
- Each profile flags reusable patterns, non-goals, and safety risks.
- No external code is copied.

## Proof Level

- This slice proves: Repo/doc inspection plus independent track summaries.

## Integration Closure

Profiles feed the cross-system synthesis in S03.

## Verification

- Creates one profile artifact per system for future reference.

## Tasks

- [x] **T01: Profile GPT Researcher and AI-Researcher** `est:90m`
  Profile GPT Researcher and AI-Researcher from authoritative sources. Focus on architecture, source acquisition, provenance/citation handling, review gates, autonomy boundaries, failure modes, and reusable/non-goal patterns for daily-archive.
  - Files: `.gsd/milestones/M019-221lb7/slices/S02/profiles/gpt-researcher-profile.md`, `.gsd/milestones/M019-221lb7/slices/S02/profiles/ai-researcher-profile.md`
  - Verify: Profile guard asserts required sections and citations.

- [x] **T02: Profile The AI Scientist and prismAId** `est:90m`
  Profile The AI Scientist and prismAId from authoritative sources. Focus on architecture, source acquisition, provenance/citation handling, review gates, autonomy boundaries, failure modes, and reusable/non-goal patterns for daily-archive.
  - Files: `.gsd/milestones/M019-221lb7/slices/S02/profiles/ai-scientist-profile.md`, `.gsd/milestones/M019-221lb7/slices/S02/profiles/prismaid-profile.md`
  - Verify: Profile guard asserts required sections and citations.

- [x] **T03: Write profile index** `est:30m`
  Run a profile completeness guard over all four profile artifacts and write an S02 profile index report summarizing profile confidence and known gaps.
  - Files: `.gsd/milestones/M019-221lb7/slices/S02/research-agent-profile-index.md`
  - Verify: uv run python inline assertions over profile files and index

## Files Likely Touched

- .gsd/milestones/M019-221lb7/slices/S02/profiles/gpt-researcher-profile.md
- .gsd/milestones/M019-221lb7/slices/S02/profiles/ai-researcher-profile.md
- .gsd/milestones/M019-221lb7/slices/S02/profiles/ai-scientist-profile.md
- .gsd/milestones/M019-221lb7/slices/S02/profiles/prismaid-profile.md
- .gsd/milestones/M019-221lb7/slices/S02/research-agent-profile-index.md
