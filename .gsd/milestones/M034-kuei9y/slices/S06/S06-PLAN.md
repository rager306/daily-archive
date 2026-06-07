# S06: Roadmap Gates and Conflict Resolution Plan

**Goal:** Create the next-stage roadmap gates and conflict-resolution plan, ensuring every remaining S01 audit clarification is routed and future implementation cannot skip universal-KB scope, GraphDB evaluation, state model, queue semantics, dependency graph, failure taxonomy, sidecar lifecycle, review boundary, graph-readiness handoff, or agent boundary gates.
**Demo:** After this, the implementation roadmap has mandatory architecture gates and every remaining R/D conflict has a correction, deferral, or user-discussion path.

## Must-Haves

- Roadmap includes gates for universal KB scope, GraphDB evaluation, state model, queue semantics, artifact dependency graph, failure taxonomy, sidecar lifecycle, review boundary, graph-readiness handoff, and agent boundary.
- Each gate lists the question, options, decision criteria, and required artifact before coding.
- Implementation slices are ordered after decision gates.
- Roadmap avoids final GraphDB selection, production graph import, and agentic orchestration unless explicitly deferred to later milestones.
- Remaining S01 audit conflicts are assigned one of: corrected by requirement update, superseded by new decision, deferred with rationale, or needs user discussion.
- R/D audit uses tables as the primary source of truth; Mermaid relationship maps are optional and small/readable.

## Proof Level

- This slice proves: Roadmap structure review, gate completeness check, and conflict routing checklist.

## Integration Closure

Prepares the next milestone handoff from documentation hardening into prototype planning while preserving unresolved-decision visibility.

## Verification

- Documents what future agents must check before starting implementation.

## Tasks

- [x] **T01: Draft roadmap gates and next milestone outline** `est:medium`
  Create ROADMAP-GATES.md and NEXT-MILESTONE-HANDOFF.md with architecture gates, options, decision criteria, required artifacts, and a suggested next prototype milestone sequence.
  - Files: `.gsd/milestones/M034-kuei9y/decision-package/ROADMAP-GATES.md`, `.gsd/milestones/M034-kuei9y/decision-package/NEXT-MILESTONE-HANDOFF.md`
  - Verify: Check roadmap gates include universal KB, GraphDB evaluation, state model, queue semantics, dependency graph, failure taxonomy, sidecar lifecycle, review boundary, readiness handoff, and agent boundary.

- [x] **T02: Draft conflict resolution and open questions register** `est:medium`
  Create CONFLICT-RESOLUTION-PLAN.md and OPEN-QUESTIONS.md routing all 15 S01 needs-clarification findings into resolved-by-ADR, deferred-with-rationale, requirement-update-needed, superseding-decision-needed, or user-discussion-needed categories.
  - Files: `.gsd/milestones/M034-kuei9y/decision-package/CONFLICT-RESOLUTION-PLAN.md`, `.gsd/milestones/M034-kuei9y/decision-package/OPEN-QUESTIONS.md`
  - Verify: Check every S01 needs-clarification route appears in the conflict-resolution plan and open questions are separate from accepted decisions.

- [x] **T03: Verify S06 roadmap gates and conflict resolution** `est:small`
  Implement and run a verifier checking all required gates, no-authorization markers, conflict route coverage, open-question separation, and next-handoff content.
  - Files: `scripts/verify_m034_roadmap_gates.py`
  - Verify: `uv run python scripts/verify_m034_roadmap_gates.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run ruff check scripts/verify_m034_roadmap_gates.py`

## Files Likely Touched

- .gsd/milestones/M034-kuei9y/decision-package/ROADMAP-GATES.md
- .gsd/milestones/M034-kuei9y/decision-package/NEXT-MILESTONE-HANDOFF.md
- .gsd/milestones/M034-kuei9y/decision-package/CONFLICT-RESOLUTION-PLAN.md
- .gsd/milestones/M034-kuei9y/decision-package/OPEN-QUESTIONS.md
- scripts/verify_m034_roadmap_gates.py
