# S07: Decision Package Closeout and Handoff

**Goal:** Audit the complete M034 decision package for final coherence, verify all S01-S06 artifacts together, document remaining open questions and risks, and prepare the package for the next tool-driven hardening or implementation-planning step.
**Demo:** After this, the package is ready for a stricter tool or next workflow to consume without losing project context.

## Must-Haves

- Cross-artifact consistency audit passes.
- Open questions are listed separately from accepted decisions.
- R/D audit corrections are either applied, deferred with rationale, or escalated for user discussion.
- Remaining risks and recommended next milestone are documented.
- Reader test confirms a human and an LLM can identify each ADR's binding decision, non-authorization boundary, impacted R/D records, contract impact, open questions, universal-KB north star, scientific-paper first-domain scope, deferred GraphDB selection, and sidecar pipeline mechanics.

## Proof Level

- This slice proves: Reader-test style verification, explicit safety invariant audit, and Mermaid readability check.

## Integration Closure

Completes the documentation package and makes it ready for the next tool-driven hardening step.

## Verification

- Provides a concise handoff surface for future agents and reviewers.

## Tasks

- [x] **T01: Create final decision package summary** `est:small`
  Create DECISION-PACKAGE-SUMMARY.md summarizing the ADR set, PRD, requirements, contracts, gates, open questions, safety invariants, and next milestone recommendation.
  - Files: `.gsd/milestones/M034-kuei9y/decision-package/DECISION-PACKAGE-SUMMARY.md`
  - Verify: Check summary references all major package artifacts and safety defaults.

- [x] **T02: Verify complete M034 decision package** `est:small`
  Implement and run a final verifier that composes all prior verifiers and checks the final summary/handoff artifacts, safety defaults, accepted/deferred ADR statuses, and package completeness.
  - Files: `scripts/verify_m034_decision_package.py`
  - Verify: `uv run python scripts/verify_m034_decision_package.py --package-dir .gsd/milestones/M034-kuei9y/decision-package --requirements .gsd/REQUIREMENTS.md --decisions .gsd/DECISIONS.md && uv run ruff check scripts/verify_m034_*.py`

## Files Likely Touched

- .gsd/milestones/M034-kuei9y/decision-package/DECISION-PACKAGE-SUMMARY.md
- scripts/verify_m034_decision_package.py
