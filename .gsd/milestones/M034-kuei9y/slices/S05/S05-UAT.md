# S05: Contracts and Invariants — UAT

**Milestone:** M034-kuei9y
**Written:** 2026-06-06T08:10:29.510Z

# S05 UAT

## Reader scenario
A future implementation agent should be able to read S05 artifacts and answer:

1. What are the generic universal-KB contracts?
2. What are the paper-specific specializations?
3. What are the required job statuses and transitions?
4. Which failures are retryable, terminal, blocked, stale, or needs_review?
5. What is the artifact dependency model?
6. What safety flags must stay false?
7. Where does the graph boundary stop?

## Expected result
- CONTRACTS includes generic contracts and paper-specific sidecar contracts.
- SAFETY-INVARIANTS includes explicit false safety defaults.
- STATUS-MATRIX includes 10 statuses.
- FAILURE-TAXONOMY includes failure classes and concrete error codes.
- ARTIFACT-DEPENDENCY-MODEL stops at GraphReadinessHandoff/no-write boundary.
- Verifier and Ruff pass.

## Verification command

```bash
uv run python scripts/verify_m034_rd_consistency_audit.py --package-dir .gsd/milestones/M034-kuei9y/decision-package --requirements .gsd/REQUIREMENTS.md --decisions .gsd/DECISIONS.md && uv run python scripts/verify_m034_adr_template_and_north_star.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run python scripts/verify_m034_formal_adr_package.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run python scripts/verify_m034_prd_requirements.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run python scripts/verify_m034_contracts_invariants.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run ruff check scripts/verify_m034_rd_consistency_audit.py scripts/verify_m034_adr_template_and_north_star.py scripts/verify_m034_formal_adr_package.py scripts/verify_m034_prd_requirements.py scripts/verify_m034_contracts_invariants.py
```
