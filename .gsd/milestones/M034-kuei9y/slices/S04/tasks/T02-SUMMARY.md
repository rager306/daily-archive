---
id: T02
parent: S04
milestone: M034-kuei9y
key_files:
  - .gsd/milestones/M034-kuei9y/decision-package/FUNCTIONAL-REQUIREMENTS.md
  - .gsd/milestones/M034-kuei9y/decision-package/NON-FUNCTIONAL-REQUIREMENTS.md
key_decisions:
  - (none)
duration: 
verification_result: passed
completed_at: 2026-06-06T08:04:26.469Z
blocker_discovered: false
---

# T02: Drafted the functional and non-functional requirements package for universal evidence orchestration.

**Drafted the functional and non-functional requirements package for universal evidence orchestration.**

## What Happened

Created `FUNCTIONAL-REQUIREMENTS.md` and `NON-FUNCTIONAL-REQUIREMENTS.md`. The functional requirements separate generic universal-KB requirements from scientific-paper first-domain requirements and safety requirements. The non-functional requirements cover local-first operation, reproducibility, redaction, observability, resumability, bounded concurrency, GraphDB portability, reviewability, fail-closed defaults, and Mermaid/readability discipline. Added explicit SFR-004 after verification caught missing safety-default wording in the functional requirements file.

## Verification

Ran a local marker check confirming both requirement documents include all safety defaults: `graph_import_allowed=false`, `graphdb_written=false`, `ladybugdb_written=false`, `production_import_attempted=false`, and `import_eligible=false`. The check also verified PRD ADR references and generic/scientific markers.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `gsd_exec purpose='M034 S04 T01-T02 verify PRD and requirements docs after safety fix'` | 0 | ✅ pass: functional/non-functional requirements include safety defaults and required scope markers | 61ms |

## Deviations

Added explicit SFR-004 after the first marker check failed; this strengthened the safety requirement rather than changing scope.

## Known Issues

None.

## Files Created/Modified

- `.gsd/milestones/M034-kuei9y/decision-package/FUNCTIONAL-REQUIREMENTS.md`
- `.gsd/milestones/M034-kuei9y/decision-package/NON-FUNCTIONAL-REQUIREMENTS.md`
