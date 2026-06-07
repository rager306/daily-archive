---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T03: Verify S05 contracts and invariants

Implement and run a verifier for contracts/invariants/status/failure/dependency artifacts, checking required contract names, safety flags, status transitions, failure classes, dependency model, GraphDB portability, and Mermaid readability limits.

## Inputs

- `.gsd/milestones/M034-kuei9y/decision-package/CONTRACTS.md`
- `.gsd/milestones/M034-kuei9y/decision-package/SAFETY-INVARIANTS.md`
- `.gsd/milestones/M034-kuei9y/decision-package/STATUS-MATRIX.md`
- `.gsd/milestones/M034-kuei9y/decision-package/FAILURE-TAXONOMY.md`
- `.gsd/milestones/M034-kuei9y/decision-package/ARTIFACT-DEPENDENCY-MODEL.md`

## Expected Output

- `scripts/verify_m034_contracts_invariants.py`

## Verification

`uv run python scripts/verify_m034_contracts_invariants.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run ruff check scripts/verify_m034_contracts_invariants.py`

## Observability Impact

Verifier reports missing contracts, missing status/failure classes, missing safety defaults, and diagram overuse.
