---
id: M017-cf3fd0
title: "MiniMax Safe Helper Implementation"
status: complete
completed_at: 2026-05-21T06:42:38.186Z
key_decisions:
  - MiniMax safe helpers are dev-only bounded helpers.
  - Usage/remains helper follows M016 9router semantics.
  - Structured helper requires Anthropic-compatible forced tool calls plus local schema validation.
  - MiniMax remains non-authoritative and cannot write/import KG data.
key_files:
  - src/arxiv_archive/minimax_usage.py
  - tests/test_minimax_usage.py
  - src/arxiv_archive/minimax_structured.py
  - tests/test_minimax_structured.py
  - .gsd/milestones/M017-cf3fd0/slices/S04/run-evidence/final-m017-guard.json
  - .gsd/milestones/M017-cf3fd0/M017-cf3fd0-VALIDATION.md
lessons_learned:
  - Do not treat app-shell-only Jina extraction as research ingestion.
  - Security review should check dataclass repr leakage for objects that may carry headers/prompts.
  - Raw corpus payloads need content marker checks in addition to caller-declared payload class.
---

# M017-cf3fd0: MiniMax Safe Helper Implementation

**M017 delivered tested dev-only MiniMax usage and structured helper boundaries while preserving all Scientific KG safety blocks.**

## What Happened

M017 converted the proven MiniMax findings into reusable, dev-only project helper code. S01 attempted the requested Manus/Jina research and documented that only the Manus app shell/CAPTCHA warning was extractable. S02 implemented the MiniMax usage/remains helper with canonical key alias resolution, 9router endpoint order, provider success checks, count semantics, and sanitized diagnostics. S03 implemented the structured helper boundary with Anthropic-compatible forced tool request construction, local schema validation, prompt-only JSON rejection, raw corpus blocking, and non-authoritative results. S04 ran independent review and security review, remediated repr leakage/raw-corpus marker risks, wrote final guard/recommendation, and validated R045.

## Success Criteria Results

All success criteria passed. Final verification: 9 targeted tests passed, ruff passed, final guard assertions passed, and independent/security review completed after remediations.

## Definition of Done Results

- [x] Manus research attempted and accessibility limitation documented.
- [x] Usage/remains helper implemented and tested.
- [x] Structured helper boundary implemented and tested.
- [x] Safety review performed and remediations applied.
- [x] R045 validated.
- [x] No production KG import/write/source-of-truth behavior enabled.

## Requirement Outcomes

R045 validated with final guard evidence. R039-R044 remain respected; no prior MiniMax/KG constraints were relaxed.

## Deviations

Manus research could not be substantively extracted via Jina; this was documented as an accessibility limitation and did not affect implementation. Security review initially flagged two helper issues, both fixed before milestone completion.

## Follow-ups

Next options: (1) comparative research-agent spike for GPT Researcher, AI-Researcher, AI Scientist, prismAId; or (2) KG candidate locators/chunk-span provenance milestone. A separate dependency-security milestone can address broader vulnerable ML packages if they matter to active runtime paths.
