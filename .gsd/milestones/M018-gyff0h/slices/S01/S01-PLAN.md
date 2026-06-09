# S01: S01

**Goal:** Capture current dependency graph, audit output, and package ownership for torch/transformers without changing dependencies.
**Demo:** After S01, dependency/audit inventory exists with package versions, advisory summary, and sanitized tool evidence.

## Must-Haves

- Direct and transitive dependency sources are identified.
- Audit findings are summarized without raw secrets or environment dumps.
- Runtime Python and lock/dependency files are recorded.
- No dependencies are changed.

## Proof Level

- This slice proves: Command evidence plus sanitized artifact review.

## Integration Closure

Inventory artifacts feed S02 reachability analysis.

## Verification

- Records commands, exit codes, and sanitized summaries for future dependency/security review.

## Tasks

- [x] **T01: Captured sanitized ML dependency inventory and transitive path from docling to torch/transformers.** `est:45m`
  Inspect Python dependency declarations and lockfile summaries for torch/transformers ownership. Run sanitized package inventory commands using uv without modifying dependencies. Write a dependency inventory artifact under S01 run-evidence.
  - Files: `pyproject.toml`, `uv.lock`, `.gsd/milestones/M018-gyff0h/slices/S01/run-evidence/dependency-inventory.json`, `.gsd/milestones/M018-gyff0h/slices/S01/dependency-inventory-report.md`
  - Verify: uv run python scripts/guard-style JSON assertions or equivalent inline assertion over dependency-inventory.json

- [x] **T02: Captured sanitized dependency audit summary for torch/transformers vulnerability debt.** `est:45m`
  Run a sanitized dependency vulnerability audit focused on current environment/dependency graph. Summarize advisory package names, counts, installed versions, and fix availability without dumping raw vulnerable payloads or environment secrets.
  - Files: `.gsd/milestones/M018-gyff0h/slices/S01/run-evidence/dependency-audit-summary.json`, `.gsd/milestones/M018-gyff0h/slices/S01/dependency-audit-report.md`
  - Verify: uv run python inline assertions over dependency-audit-summary.json

## Files Likely Touched

- pyproject.toml
- uv.lock
- .gsd/milestones/M018-gyff0h/slices/S01/run-evidence/dependency-inventory.json
- .gsd/milestones/M018-gyff0h/slices/S01/dependency-inventory-report.md
- .gsd/milestones/M018-gyff0h/slices/S01/run-evidence/dependency-audit-summary.json
- .gsd/milestones/M018-gyff0h/slices/S01/dependency-audit-report.md
