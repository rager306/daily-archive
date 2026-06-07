# S04: PRD and Requirement Package — UAT

**Milestone:** M034-kuei9y
**Written:** 2026-06-06T08:06:10.973Z

# S04 UAT

## Reader scenario
A future implementation agent should be able to read the S04 package and answer:

1. What product is being built? Universal evidence orchestration for a local-first KB.
2. What is the first domain? Scientific articles.
3. What is generic versus paper-specific?
4. What is explicitly out of scope?
5. Which functional requirements define queue/status/retry/lazy/dependency/review/readiness/safety behavior?
6. Which non-functional requirements constrain locality, reproducibility, redaction, observability, resumability, concurrency, GraphDB portability, and fail-closed defaults?

## Expected result
- PRD references ADR-000/002/003/004/005/006/007.
- Functional requirements include FR/PFR/SFR sections and 20 functional/safety IDs.
- Non-functional requirements include 10 NFR IDs.
- Safety defaults are explicit in all three docs.
- Verifier and Ruff pass.

## Verification command

```bash
uv run python scripts/verify_m034_rd_consistency_audit.py --package-dir .gsd/milestones/M034-kuei9y/decision-package --requirements .gsd/REQUIREMENTS.md --decisions .gsd/DECISIONS.md && uv run python scripts/verify_m034_adr_template_and_north_star.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run python scripts/verify_m034_formal_adr_package.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run python scripts/verify_m034_prd_requirements.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run ruff check scripts/verify_m034_rd_consistency_audit.py scripts/verify_m034_adr_template_and_north_star.py scripts/verify_m034_formal_adr_package.py scripts/verify_m034_prd_requirements.py
```
