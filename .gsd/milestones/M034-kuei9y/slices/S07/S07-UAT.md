# S07: Decision Package Closeout and Handoff — UAT

**Milestone:** M034-kuei9y
**Written:** 2026-06-06T08:17:03.277Z

# S07 UAT

## Reader scenario
A future human or LLM agent should be able to open `DECISION-PACKAGE-SUMMARY.md` and know:

1. The project is a local-first universal KB with scientific articles as first domain.
2. Which ADRs are accepted and which are deferred.
3. Which files make up the decision package.
4. What safety defaults are binding.
5. What must not be inferred or implemented yet.
6. What next milestone is recommended.
7. Which single command verifies the whole package.

## Expected result
- Summary references all major package artifacts.
- Final verifier reports 22 package files and six sub-verifiers.
- Ruff passes for all M034 verifier scripts.

## Verification command

```bash
uv run python scripts/verify_m034_decision_package.py --package-dir .gsd/milestones/M034-kuei9y/decision-package --requirements .gsd/REQUIREMENTS.md --decisions .gsd/DECISIONS.md && uv run ruff check scripts/verify_m034_*.py
```
