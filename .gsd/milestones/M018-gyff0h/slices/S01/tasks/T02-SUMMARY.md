---
id: T02
parent: S01
milestone: M018-gyff0h
key_files:
  - .gsd/milestones/M018-gyff0h/slices/S01/run-evidence/dependency-audit-summary.json
  - .gsd/milestones/M018-gyff0h/slices/S01/dependency-audit-report.md
key_decisions: []
duration: 
verification_result: passed
completed_at: 2026-05-21T07:00:05.862Z
blocker_discovered: false
---

# T02: Captured sanitized dependency audit summary for torch/transformers vulnerability debt.

**Captured sanitized dependency audit summary for torch/transformers vulnerability debt.**

## What Happened

Ran `uv run --with pip-audit pip-audit -f json --progress-spinner off` and summarized findings without persisting raw advisory details. The sanitized summary records 2 vulnerable packages and 19 total vulnerability findings: torch 2.12.0 with 11 and transformers 5.8.1 with 8. No dependency changes were made.

## Verification

Inline JSON guard passed: vulnerable_dependency_count=2, total_vulnerability_count=19, raw_audit_json_persisted=false, secrets_logged=false, raw_corpus_payload_logged=false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run --with pip-audit pip-audit -f json --progress-spinner off` | 1 | ✅ expected non-zero — findings summarized safely | 0ms |
| 2 | `uv run python inline assertions over dependency-audit-summary.json` | 0 | ✅ pass — m018-s01-inventory-audit-guard-ok | 4200ms |

## Deviations

The raw pip-audit JSON was generated transiently, parsed into a sanitized summary, and removed rather than persisted.

## Known Issues

Audit reports 19 vulnerabilities across torch and transformers with no fix versions reported by pip-audit. Reachability and exploitability require S02/S03 analysis before severity decisions.

## Files Created/Modified

- `.gsd/milestones/M018-gyff0h/slices/S01/run-evidence/dependency-audit-summary.json`
- `.gsd/milestones/M018-gyff0h/slices/S01/dependency-audit-report.md`
