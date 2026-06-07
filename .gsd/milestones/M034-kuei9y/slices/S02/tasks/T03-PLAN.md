---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T03: Verify S02 ADR package

Implement and run a verifier for the ADR template/index/north-star package, checking template sections, ADR-000 sections, safety markers, R/D references, Mermaid readability constraints, and S01 audit consumption.

## Inputs

- `.gsd/milestones/M034-kuei9y/decision-package/ADR-TEMPLATE.md`
- `.gsd/milestones/M034-kuei9y/decision-package/ADR-INDEX.md`
- `.gsd/milestones/M034-kuei9y/decision-package/ADR-000-universal-kb-north-star.md`
- `.gsd/milestones/M034-kuei9y/decision-package/r-d-consistency-audit.json`

## Expected Output

- `scripts/verify_m034_adr_template_and_north_star.py`

## Verification

`uv run python scripts/verify_m034_adr_template_and_north_star.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run ruff check scripts/verify_m034_adr_template_and_north_star.py`

## Observability Impact

Verifier reports missing sections, missing safety markers, missing R/D references, and diagram overuse.
