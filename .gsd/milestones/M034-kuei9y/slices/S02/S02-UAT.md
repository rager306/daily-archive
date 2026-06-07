# S02: ADR Template and Universal KB North Star — UAT

**Milestone:** M034-kuei9y
**Written:** 2026-06-06T07:54:47.949Z

# S02 UAT

## Reader scenario
A future human or LLM agent should be able to read the S02 package and answer:

1. Which ADR template is binding for M034?
2. Which ADRs are planned and what status vocabulary applies?
3. What is the project north star?
4. Is daily-archive paper-only or universal-KB with papers first?
5. Is GraphDB selection final?
6. Does ADR-000 authorize graph writes, parser-as-truth, or agent orchestration?

## Expected result
- ADR index points to `.gsd/milestones/M034-kuei9y/decision-package/ADR-TEMPLATE.md`.
- ADR template includes sections 0–14 and Mermaid readability rules.
- ADR-000 is `Accepted` and `binding`.
- ADR-000 states local-first universal KB with scientific articles as the primary first domain.
- ADR-000 explicitly defers GraphDB selection and blocks GraphDB writes/import.
- ADR-000 includes LLM Reading Notes.
- Verifier and Ruff pass.

## Verification command

```bash
uv run python scripts/verify_m034_rd_consistency_audit.py --package-dir .gsd/milestones/M034-kuei9y/decision-package --requirements .gsd/REQUIREMENTS.md --decisions .gsd/DECISIONS.md && uv run python scripts/verify_m034_adr_template_and_north_star.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run ruff check scripts/verify_m034_rd_consistency_audit.py scripts/verify_m034_adr_template_and_north_star.py
```
