# S06: Roadmap Gates and Conflict Resolution Plan — UAT

**Milestone:** M034-kuei9y
**Written:** 2026-06-06T08:14:07.283Z

# S06 UAT

## Reader scenario
A future implementation agent should be able to read S06 artifacts and answer:

1. Which architecture gates must be resolved before coding?
2. Which options and decision criteria apply to each gate?
3. What must not be implemented yet?
4. Which S01 clarifications were routed?
5. Which questions remain open and are not accepted decisions?

## Expected result
- ROADMAP-GATES includes 10 required gates.
- CONFLICT-RESOLUTION-PLAN covers all 15 S01 needs-clarification routes.
- OPEN-QUESTIONS separates unresolved questions from accepted decisions.
- NEXT-MILESTONE-HANDOFF lists ready inputs, recommended prototype slices, and must-not-implement items.
- Verifier and Ruff pass.

## Verification command

```bash
uv run python scripts/verify_m034_rd_consistency_audit.py --package-dir .gsd/milestones/M034-kuei9y/decision-package --requirements .gsd/REQUIREMENTS.md --decisions .gsd/DECISIONS.md && uv run python scripts/verify_m034_adr_template_and_north_star.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run python scripts/verify_m034_formal_adr_package.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run python scripts/verify_m034_prd_requirements.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run python scripts/verify_m034_contracts_invariants.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run python scripts/verify_m034_roadmap_gates.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run ruff check scripts/verify_m034_rd_consistency_audit.py scripts/verify_m034_adr_template_and_north_star.py scripts/verify_m034_formal_adr_package.py scripts/verify_m034_prd_requirements.py scripts/verify_m034_contracts_invariants.py scripts/verify_m034_roadmap_gates.py
```
