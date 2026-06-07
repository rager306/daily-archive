---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T04: Verify S01 audit package

Implement and run a local verifier for the S01 audit package, checking source coverage, classification coverage, conflict routing, safety invariant presence, and no silent mutation policy. Produce verifier output suitable for GSD closeout.

## Inputs

- `.gsd/milestones/M034-kuei9y/decision-package/r-d-inventory.json`
- `.gsd/milestones/M034-kuei9y/decision-package/r-d-consistency-audit.json`
- `.gsd/milestones/M034-kuei9y/decision-package/correction-checklist.md`
- `.gsd/milestones/M034-kuei9y/decision-package/open-conflicts-for-user.md`

## Expected Output

- `scripts/verify_m034_rd_consistency_audit.py`

## Verification

`uv run python scripts/verify_m034_rd_consistency_audit.py --package-dir .gsd/milestones/M034-kuei9y/decision-package --requirements .gsd/REQUIREMENTS.md --decisions .gsd/DECISIONS.md`

## Observability Impact

Verifier reports coverage counts, missing classifications, unrouted conflicts, and safety invariant checks.
