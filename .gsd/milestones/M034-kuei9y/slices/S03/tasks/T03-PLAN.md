---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T03: Verify S03 formal ADR package

Implement and run a verifier for all S03 ADRs and the ADR index, checking template sections, status/binding levels, GraphDB deferral, safety markers, R/D references, Mermaid limits, and S01 audit route coverage.

## Inputs

- `.gsd/milestones/M034-kuei9y/decision-package/ADR-INDEX.md`
- `.gsd/milestones/M034-kuei9y/decision-package/ADR-000-universal-kb-north-star.md`
- `.gsd/milestones/M034-kuei9y/decision-package/r-d-consistency-audit.json`

## Expected Output

- `scripts/verify_m034_formal_adr_package.py`

## Verification

`uv run python scripts/verify_m034_formal_adr_package.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run ruff check scripts/verify_m034_formal_adr_package.py`

## Observability Impact

Verifier reports missing ADRs, section gaps, overused diagrams, missing safety defaults, and missing R/D references.
