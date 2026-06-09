# S01: S01

**Goal:** Identify authoritative repositories/docs for GPT Researcher, AI-Researcher, The AI Scientist, and prismAId, and capture basic project metadata.
**Demo:** After S01, the project has a verified source map for the four target systems with repo/doc URLs and extraction feasibility.

## Must-Haves

- Each target has a source URL or explicit not-found status.
- Search queries and source URLs are recorded.
- Evidence distinguishes docs, repos, papers, and secondary descriptions.
- No implementation is changed.

## Proof Level

- This slice proves: Web/repo evidence with sanitized citations.

## Integration Closure

Source map feeds deeper per-system analysis in S02.

## Verification

- Records exact URLs, search queries used, and source classifications.

## Tasks

- [x] **T01: Mapped GPT Researcher and AI-Researcher authoritative sources.** `est:45m`
  Search for authoritative sources for GPT Researcher and AI-Researcher. Capture repo/doc/paper URLs, ownership signals, license if visible, and source confidence.
  - Files: `.gsd/milestones/M019-221lb7/slices/S01/run-evidence/gpt-researcher-source-map.json`, `.gsd/milestones/M019-221lb7/slices/S01/run-evidence/ai-researcher-source-map.json`
  - Verify: JSON source-map assertions for status/source URLs/confidence

- [x] **T02: Mapped The AI Scientist and prismAId authoritative sources and disambiguated prismAId.** `est:45m`
  Search for authoritative sources for The AI Scientist and prismAId. Capture repo/doc/paper URLs, ownership signals, license if visible, and source confidence.
  - Files: `.gsd/milestones/M019-221lb7/slices/S01/run-evidence/ai-scientist-source-map.json`, `.gsd/milestones/M019-221lb7/slices/S01/run-evidence/prismaid-source-map.json`
  - Verify: JSON source-map assertions for status/source URLs/confidence

- [x] **T03: Wrote consolidated research-agent source-map report.** `est:30m`
  Combine individual source maps into a human-readable S01 source-map report with caveats and source confidence. Do not profile architectures yet beyond source identification.
  - Files: `.gsd/milestones/M019-221lb7/slices/S01/research-agent-source-map.md`
  - Verify: Guard asserts all four targets have source-map entries and no implementation files changed.

## Files Likely Touched

- .gsd/milestones/M019-221lb7/slices/S01/run-evidence/gpt-researcher-source-map.json
- .gsd/milestones/M019-221lb7/slices/S01/run-evidence/ai-researcher-source-map.json
- .gsd/milestones/M019-221lb7/slices/S01/run-evidence/ai-scientist-source-map.json
- .gsd/milestones/M019-221lb7/slices/S01/run-evidence/prismaid-source-map.json
- .gsd/milestones/M019-221lb7/slices/S01/research-agent-source-map.md
