---
id: T01
parent: S01
milestone: M021-xcfj4p
key_files:
  - .gsd/milestones/M021-xcfj4p/slices/S01/deterministic-locator-design.md
key_decisions: []
duration: 
verification_result: passed
completed_at: 2026-05-21T10:10:55.597Z
blocker_discovered: false
---

# T01: Designed the deterministic candidate locator implementation boundary.

**Designed the deterministic candidate locator implementation boundary.**

## What Happened

Inspected existing evidence, import-boundary, and validation-batch patterns and drafted the deterministic locator implementation design. The design proposes an additive module `src/arxiv_archive/candidate_locators.py` plus tests, with explicit data model, source ledgers, spans, locator records, ambiguity diagnostics, recursive forbidden-key validation, safety flags, and no-import semantics.

## Verification

Verified with uv run python inline design assertions. Guard returned m021-s01-design-impact-guard-ok.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python inline S01 design/impact guard` | 0 | ✅ pass: m021-s01-design-impact-guard-ok | 10200ms |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `.gsd/milestones/M021-xcfj4p/slices/S01/deterministic-locator-design.md`
