---
id: M019-221lb7
title: "Open Source Research Agent Comparative Spike"
status: complete
completed_at: 2026-05-21T07:52:50.016Z
key_decisions:
  - prismAId is the primary positive pattern source for protocol-bound review workflows.
  - GPT Researcher is the secondary pattern source for bounded orchestration and source tracking.
  - AI-Researcher and The AI Scientist are cautionary non-goal sources for autonomy boundaries.
  - The next milestone should return to Scientific KG candidate locators and chunk-span provenance.
key_files:
  - .gsd/milestones/M019-221lb7/slices/S01/research-agent-source-map.md
  - .gsd/milestones/M019-221lb7/slices/S02/research-agent-profile-index.md
  - .gsd/milestones/M019-221lb7/slices/S03/research-agent-comparative-matrix.md
  - .gsd/milestones/M019-221lb7/slices/S03/run-evidence/final-research-agent-spike-guard.json
  - .gsd/milestones/M019-221lb7/M019-221lb7-VALIDATION.md
lessons_learned:
  - Disambiguation matters: prismAId is Open-and-Sustainable/prismAId, not Prismer-AI/Prismer.
  - Protocol-bound systematic review tools are a better fit for daily-archive than autonomous scientist systems.
  - High-autonomy research-agent projects are useful safety boundary examples even when not adoption targets.
---

# M019-221lb7: Open Source Research Agent Comparative Spike

**M019 completed the open-source research-agent comparative spike and recommended protocol-bound KG candidate locator work next.**

## What Happened

M019 compared GPT Researcher, AI-Researcher, The AI Scientist, and prismAId at the pattern level. S01 found authoritative sources and disambiguated prismAId. S02 produced profiles covering architecture, source acquisition, provenance, review gates, autonomy, failure modes, reusable patterns, and non-goals. S03 synthesized those profiles into a comparative matrix and final recommendation, then passed independent review. R047 was validated. No third-party code was copied, no dependencies were adopted, and no production KG import or LadybugDB write path was enabled.

## Success Criteria Results

All success criteria passed. The next KG/provenance milestone is clearer: use prismAId-like protocol/source-ledger/review gates and GPT Researcher-like bounded orchestration, while rejecting autonomous scientist behavior.

## Definition of Done Results

- [x] Source maps created for all four systems.
- [x] Profiles created for all four systems.
- [x] Comparative matrix and final recommendation written.
- [x] Independent review passed.
- [x] R047 validated.
- [x] No external code/dependency adoption or KG production activation occurred.

## Requirement Outcomes

R047 validated with source maps, profiles, comparative matrix, final guard, and independent review evidence.

## Deviations

None. The spike remained pattern-level as planned.

## Follow-ups

Plan `KG Candidate Locator and Chunk-Span Provenance Protocol` as the next milestone. Suggested slices: protocol contract, one-paper locator fixture, small-batch locator rehearsal, independent semantic review.
