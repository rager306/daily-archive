# S01: R and D Conflict Audit First — UAT

**Milestone:** M034-kuei9y
**Written:** 2026-06-06T07:49:01.171Z

# S01 UAT

## Reader scenario
A future agent preparing M034 ADRs should be able to open the S01 package and answer:

1. How many current requirements and decisions exist?
2. Which records are binding/consistent, historical-only, or need clarification?
3. Which old records mention LadybugDB, paper-domain scope, sidecar scope, or broad helper/agent scope?
4. Which clarifications must be reflected in ADRs, PRD, contracts, or roadmap gates?
5. Are there any blocking conflicts needing immediate user decision?

## Expected result
- Inventory reports 61 requirements and 67 decisions with no duplicate IDs.
- Audit reports 128 classified records.
- Classification counts are 35 consistent, 78 historical-scope-only, 15 needs-clarification.
- Open-conflicts report states there are no immediate `conflict-needs-user-decision` records after false-positive refinement.
- Correction checklist routes all 15 clarification records.
- Verifier command passes and Ruff passes.

## Verification command

```bash
uv run python scripts/verify_m034_rd_consistency_audit.py --package-dir .gsd/milestones/M034-kuei9y/decision-package --requirements .gsd/REQUIREMENTS.md --decisions .gsd/DECISIONS.md && uv run ruff check scripts/verify_m034_rd_consistency_audit.py
```
