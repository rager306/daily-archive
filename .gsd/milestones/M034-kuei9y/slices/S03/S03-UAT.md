# S03: Formal ADR Package and GraphDB Deferral — UAT

**Milestone:** M034-kuei9y
**Written:** 2026-06-06T08:01:52.515Z

# S03 UAT

## Reader scenario
A future agent should be able to open the ADR package and answer:

1. Is GraphDB selection final? No, ADR-002 defers it.
2. Are GraphDB writes allowed? No, ADR-005 blocks direct extractor/parser/sidecar/LLM writes.
3. Is durable sidecar processing supposed to be in-memory batch? No, ADR-003 requires durable lazy orchestration.
4. Are sidecar outputs graph-ready? No, ADR-004 says candidate evidence only.
5. Can agents orchestrate now? No, ADR-006 says optional future helpers only.
6. Is quant-mind adopted? No, ADR-007 says pattern source, not runtime dependency.

## Expected result
- ADR-INDEX marks ADR-000, ADR-003, ADR-004, ADR-005, ADR-006, ADR-007 Accepted and ADR-002 Deferred.
- Every formal ADR includes sections 0–14, safety defaults, and LLM Reading Notes.
- Verifier confirms 7 ADR files and Ruff passes.

## Verification command

```bash
uv run python scripts/verify_m034_rd_consistency_audit.py --package-dir .gsd/milestones/M034-kuei9y/decision-package --requirements .gsd/REQUIREMENTS.md --decisions .gsd/DECISIONS.md && uv run python scripts/verify_m034_adr_template_and_north_star.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run python scripts/verify_m034_formal_adr_package.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run ruff check scripts/verify_m034_rd_consistency_audit.py scripts/verify_m034_adr_template_and_north_star.py scripts/verify_m034_formal_adr_package.py
```
