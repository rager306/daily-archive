---
id: S01
parent: M019-221lb7
milestone: M019-221lb7
provides:
  - Four source maps
  - Consolidated source-map report
  - S02 profiling targets
requires:
  []
affects:
  []
key_files:
  - .gsd/milestones/M019-221lb7/slices/S01/research-agent-source-map.md
key_decisions:
  - Treat Open-and-Sustainable/prismAId as the prismAId target, not Prismer-AI/Prismer.
  - Do not copy third-party code; use source maps only as evidence for profiling.
patterns_established:
  - Disambiguate similarly named systems before profiling.
  - Use raw README/LICENSE sources where possible instead of relying only on search snippets.
observability_surfaces:
  - source-map JSON files
  - research-agent-source-map.md
drill_down_paths:
  - .gsd/milestones/M019-221lb7/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M019-221lb7/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M019-221lb7/slices/S01/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-21T07:31:54.840Z
blocker_discovered: false
---

# S01: Research-agent source map

**S01 established authoritative sources for the four research-agent spike targets.**

## What Happened

S01 identified authoritative source maps for GPT Researcher, AI-Researcher, The AI Scientist, and prismAId. Each target has a repository and source confidence. GPT Researcher has Apache-2.0 license evidence; The AI Scientist has a custom Responsible-AI-style license; prismAId has AGPL-3.0 evidence; AI-Researcher license was not found by raw root license fetch. The report records search queries and disambiguates prismAId from Prismer.

## Verification

Source-map guard passed.

## Requirements Advanced

- R047 — Establishes source evidence needed to compare the four systems.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

None.

## Known Limitations

AI-Researcher root LICENSE was not found; license remains unknown from S01. Some GitHub pages were truncated by fetch, but raw README/LICENSE evidence was captured where available.

## Follow-ups

S02 should inspect repo/docs evidence for architecture profiles and focus on reusable patterns vs non-goals.

## Files Created/Modified

- `.gsd/milestones/M019-221lb7/slices/S01/run-evidence/gpt-researcher-source-map.json` — GPT Researcher source map.
- `.gsd/milestones/M019-221lb7/slices/S01/run-evidence/ai-researcher-source-map.json` — AI-Researcher source map.
- `.gsd/milestones/M019-221lb7/slices/S01/run-evidence/ai-scientist-source-map.json` — The AI Scientist source map.
- `.gsd/milestones/M019-221lb7/slices/S01/run-evidence/prismaid-source-map.json` — prismAId source map.
- `.gsd/milestones/M019-221lb7/slices/S01/research-agent-source-map.md` — Combined source-map report.
