---
id: S01
parent: M018-gyff0h
milestone: M018-gyff0h
provides:
  - Dependency inventory
  - Vulnerability count summary
  - Transitive ownership path
requires:
  []
affects:
  []
key_files:
  - .gsd/milestones/M018-gyff0h/slices/S01/run-evidence/dependency-inventory.json
  - .gsd/milestones/M018-gyff0h/slices/S01/run-evidence/dependency-audit-summary.json
key_decisions:
  - Do not perform blind dependency upgrades in M018/S01.
  - Treat torch/transformers findings as transitive via docling until reachability proves otherwise.
patterns_established:
  - For dependency audits, persist sanitized package/version/count metadata, not raw audit output.
  - Interpret vulnerability counts only after reachability analysis.
observability_surfaces:
  - dependency-inventory.json
  - dependency-audit-summary.json
drill_down_paths:
  - .gsd/milestones/M018-gyff0h/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M018-gyff0h/slices/S01/tasks/T02-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-21T07:00:40.417Z
blocker_discovered: false
---

# S01: Dependency inventory and audit evidence

**S01 identified torch/transformers as vulnerable transitive ML dependencies through docling.**

## What Happened

S01 captured a read-only dependency inventory and sanitized vulnerability audit. The direct dependency path is `arxiv-daily-archive -> docling -> docling-ibm-models -> torch/transformers`. Current audit findings are 19 vulnerabilities in 2 transitive packages: torch 2.12.0 and transformers 5.8.1. No dependency files changed, raw audit JSON was not persisted, and no secrets/raw corpus data were logged.

## Verification

Inline guard over dependency-inventory.json and dependency-audit-summary.json passed.

## Requirements Advanced

- R046 — Provides initial inventory/audit evidence needed for vulnerability triage.

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

pip-audit reported no fix versions for the vulnerable torch/transformers versions; S03 may still recommend updating, isolation, or deferral after reachability analysis.

## Follow-ups

S02 must determine whether torch/transformers are reachable in active runtime paths and whether untrusted inputs can reach them.

## Files Created/Modified

- `.gsd/milestones/M018-gyff0h/slices/S01/run-evidence/dependency-inventory.json` — Sanitized dependency graph inventory.
- `.gsd/milestones/M018-gyff0h/slices/S01/dependency-inventory-report.md` — Human-readable inventory report.
- `.gsd/milestones/M018-gyff0h/slices/S01/run-evidence/dependency-audit-summary.json` — Sanitized vulnerability audit summary.
- `.gsd/milestones/M018-gyff0h/slices/S01/dependency-audit-report.md` — Human-readable audit report.
