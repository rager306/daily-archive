---
id: T01
parent: S03
milestone: M034-kuei9y
key_files:
  - .gsd/milestones/M034-kuei9y/decision-package/ADR-002-defer-final-graphdb-selection.md
  - .gsd/milestones/M034-kuei9y/decision-package/ADR-005-no-direct-extractor-to-graphdb-path.md
  - .gsd/milestones/M034-kuei9y/decision-package/ADR-INDEX.md
key_decisions:
  - (none)
duration: 
verification_result: passed
completed_at: 2026-06-06T07:57:18.339Z
blocker_discovered: false
---

# T01: Drafted the GraphDB deferral and no-direct-GraphDB-write ADRs.

**Drafted the GraphDB deferral and no-direct-GraphDB-write ADRs.**

## What Happened

Created `ADR-002-defer-final-graphdb-selection.md` and `ADR-005-no-direct-extractor-to-graphdb-path.md`. ADR-002 marks final GraphDB selection as deferred and requires future comparison across LadybugDB, FalkorDB, HelixDB, and other candidates using a backend-neutral `KnowledgeSubstratePort`. ADR-005 accepts a binding no-direct-write rule: parser, extractor, sidecar, adapter, or LLM outputs must pass candidate, validation, review, and readiness boundaries before any future GraphDB promotion. Updated the ADR index statuses for ADR-002 and ADR-005.

## Verification

Ran marker checks during generation. Both ADRs include required template sections, R/D impact, safety non-authorization, safety flags, LLM Reading Notes, and Mermaid diagram counts within limits; the ADR index was updated.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `gsd_exec purpose='M034 S03 T01 draft GraphDB and no-direct-write ADRs'` | 0 | ✅ pass: ADR-002 and ADR-005 created and marker checks passed | 160ms |

## Deviations

None.

## Known Issues

A full formal ADR package verifier is still pending T03.

## Files Created/Modified

- `.gsd/milestones/M034-kuei9y/decision-package/ADR-002-defer-final-graphdb-selection.md`
- `.gsd/milestones/M034-kuei9y/decision-package/ADR-005-no-direct-extractor-to-graphdb-path.md`
- `.gsd/milestones/M034-kuei9y/decision-package/ADR-INDEX.md`
