---
id: T01
parent: S01
milestone: M018-gyff0h
key_files:
  - .gsd/milestones/M018-gyff0h/slices/S01/run-evidence/dependency-inventory.json
  - .gsd/milestones/M018-gyff0h/slices/S01/dependency-inventory-report.md
key_decisions: []
duration: 
verification_result: passed
completed_at: 2026-05-21T06:59:44.671Z
blocker_discovered: false
---

# T01: Captured sanitized ML dependency inventory and transitive path from docling to torch/transformers.

**Captured sanitized ML dependency inventory and transitive path from docling to torch/transformers.**

## What Happened

Inspected `pyproject.toml`, parsed `uv.lock`, and captured installed package versions via `uv pip list --format json`. The direct runtime dependency source is `docling>=2.93.0`; torch/transformers are transitive through `docling-ibm-models` and related packages. No dependency files were changed.

## Verification

Inline JSON guard passed: `m018-s01-inventory-audit-guard-ok`. Inventory confirms dependencies_changed=false, secrets_logged=false, and raw_corpus_payload_logged=false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python inventory script` | 0 | ✅ pass — dependency-inventory-ok | 0ms |
| 2 | `uv run python inline assertions over dependency inventory` | 0 | ✅ pass — m018-s01-inventory-audit-guard-ok | 4200ms |

## Deviations

None.

## Known Issues

None for inventory. Vulnerability counts are handled by T02 and S02/S03.

## Files Created/Modified

- `.gsd/milestones/M018-gyff0h/slices/S01/run-evidence/dependency-inventory.json`
- `.gsd/milestones/M018-gyff0h/slices/S01/dependency-inventory-report.md`
