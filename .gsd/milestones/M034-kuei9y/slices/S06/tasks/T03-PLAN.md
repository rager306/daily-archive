---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T03: Verify S06 roadmap gates and conflict resolution

Implement and run a verifier checking all required gates, no-authorization markers, conflict route coverage, open-question separation, and next-handoff content.

## Inputs

- `.gsd/milestones/M034-kuei9y/decision-package/ROADMAP-GATES.md`
- `.gsd/milestones/M034-kuei9y/decision-package/CONFLICT-RESOLUTION-PLAN.md`
- `.gsd/milestones/M034-kuei9y/decision-package/OPEN-QUESTIONS.md`
- `.gsd/milestones/M034-kuei9y/decision-package/NEXT-MILESTONE-HANDOFF.md`

## Expected Output

- `scripts/verify_m034_roadmap_gates.py`

## Verification

`uv run python scripts/verify_m034_roadmap_gates.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run ruff check scripts/verify_m034_roadmap_gates.py`

## Observability Impact

Verifier reports missing gates or unrouted clarification findings.
