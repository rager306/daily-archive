# S04: PRD and Requirement Package

**Goal:** Write the PRD and functional/non-functional requirement package for lazy async evidence orchestration as a generic universal-KB capability with scientific-paper first-domain adapters, grounded in S01 audit and S02/S03 ADRs.
**Demo:** After this, the next implementation track has product scope, users, workflows, goals, non-goals, and functional requirements tied to the universal evidence pipeline.

## Must-Haves

- Defines users and workflows for generic knowledge evidence processing plus scientific article sidecar execution, review, recovery, and status inspection.
- Enumerates functional requirements for queueing, lazy recomputation, dependency readiness, retries, status inspection, sidecar workers, review packet generation, graph-readiness handoff, and safety flags.
- Separates generic requirements from paper-specific requirements.
- Enumerates non-functional requirements for local-first operation, reproducibility, redaction, observability, bounded concurrency, GraphDB portability, and resumability.
- Includes acceptance criteria for each requirement.
- Addresses S01 audit conflicts by either incorporating corrections, deferring with rationale, or flagging user decisions.

## Proof Level

- This slice proves: Requirements coverage review against S01 audit, R024/R027/R029/R040/R050/R054-R061, D067, and M033 constraints.

## Integration Closure

Provides implementation-ready scope for contract design and roadmap slices.

## Verification

- Requires status and failure visibility surfaces as first-class product capabilities.

## Tasks

- [x] **T01: Draft PRD for universal evidence orchestration** `est:medium`
  Create PRD.md describing the product scope, users, workflows, goals, non-goals, acceptance criteria, and safety boundaries for universal-KB evidence orchestration with scientific-paper first-domain adapters.
  - Files: `.gsd/milestones/M034-kuei9y/decision-package/PRD.md`
  - Verify: Check PRD includes goals, non-goals, users, workflows, safety boundaries, generic vs paper-specific scope, and references ADR-000/002/003/004/005/006/007.

- [x] **T02: Draft functional and non-functional requirements** `est:medium`
  Create FUNCTIONAL-REQUIREMENTS.md and NON-FUNCTIONAL-REQUIREMENTS.md. Separate generic universal-KB requirements from scientific-paper first-domain requirements and include acceptance criteria.
  - Files: `.gsd/milestones/M034-kuei9y/decision-package/FUNCTIONAL-REQUIREMENTS.md`, `.gsd/milestones/M034-kuei9y/decision-package/NON-FUNCTIONAL-REQUIREMENTS.md`
  - Verify: Check requirement docs include queue/status/retry/lazy/dependency/review/readiness/safety items, local-first/reproducibility/redaction/observability/GraphDB portability/resumability NFRs, and acceptance criteria.

- [x] **T03: Verify S04 PRD and requirements** `est:small`
  Implement and run a verifier for PRD and requirement artifacts, checking required sections, ADR references, generic/paper split, safety markers, acceptance criteria, and S01 audit clarification coverage.
  - Files: `scripts/verify_m034_prd_requirements.py`
  - Verify: `uv run python scripts/verify_m034_prd_requirements.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run ruff check scripts/verify_m034_prd_requirements.py`

## Files Likely Touched

- .gsd/milestones/M034-kuei9y/decision-package/PRD.md
- .gsd/milestones/M034-kuei9y/decision-package/FUNCTIONAL-REQUIREMENTS.md
- .gsd/milestones/M034-kuei9y/decision-package/NON-FUNCTIONAL-REQUIREMENTS.md
- scripts/verify_m034_prd_requirements.py
