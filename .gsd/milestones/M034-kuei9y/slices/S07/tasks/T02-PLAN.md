---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T02: Added and passed the final verifier for the complete M034 decision package.

Implement and run a final verifier that composes all prior verifiers and checks the final summary/handoff artifacts, safety defaults, accepted/deferred ADR statuses, and package completeness.

## Inputs

- `.gsd/milestones/M034-kuei9y/decision-package/DECISION-PACKAGE-SUMMARY.md`

## Expected Output

- `scripts/verify_m034_decision_package.py`

## Verification

`uv run python scripts/verify_m034_decision_package.py --package-dir .gsd/milestones/M034-kuei9y/decision-package --requirements .gsd/REQUIREMENTS.md --decisions .gsd/DECISIONS.md && uv run ruff check scripts/verify_m034_*.py`

## Observability Impact

Final verifier gives one command for future agents to validate the whole package.
