---
id: S04
parent: M034-kuei9y
milestone: M034-kuei9y
provides:
  - PRD for universal evidence orchestration
  - Functional requirements with acceptance criteria
  - Non-functional requirements with acceptance criteria
  - Verifier for PRD/requirements package
requires:
  []
affects:
  []
key_files:
  - .gsd/milestones/M034-kuei9y/decision-package/PRD.md
  - .gsd/milestones/M034-kuei9y/decision-package/FUNCTIONAL-REQUIREMENTS.md
  - .gsd/milestones/M034-kuei9y/decision-package/NON-FUNCTIONAL-REQUIREMENTS.md
  - scripts/verify_m034_prd_requirements.py
key_decisions:
  - Generic universal-KB requirements and scientific-paper first-domain requirements are separate.
  - Safety defaults must be explicit in PRD and requirements, not implied.
patterns_established:
  - PRD references source ADRs directly.
  - Functional requirements split generic, paper-specific, and safety requirements.
  - NFRs include GraphDB portability and fail-closed defaults.
observability_surfaces:
  - verify_m034_prd_requirements.py PRD/requirements diagnostics
drill_down_paths:
  - .gsd/milestones/M034-kuei9y/slices/S04/tasks/T01-SUMMARY.md
  - .gsd/milestones/M034-kuei9y/slices/S04/tasks/T02-SUMMARY.md
  - .gsd/milestones/M034-kuei9y/slices/S04/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-06T08:06:10.973Z
blocker_discovered: false
---

# S04: PRD and Requirement Package

**Converted the M034 ADR decisions into a PRD and generic plus paper-specific requirement package.**

## What Happened

S04 translated the accepted/deferred ADRs into implementation-facing product scope and requirements. `PRD.md` defines universal evidence orchestration as the product scope, with scientific articles as the first proving domain and no production graph import, no final GraphDB selection, no direct GraphDB writes, no parser-as-truth, and no agentic orchestration. `FUNCTIONAL-REQUIREMENTS.md` separates generic universal-KB requirements, scientific-paper first-domain requirements, and safety requirements. `NON-FUNCTIONAL-REQUIREMENTS.md` captures local-first operation, reproducibility, redaction, observability, resumability, bounded concurrency, GraphDB portability, reviewability, fail-closed defaults, and Mermaid/readability discipline. A verifier now checks the PRD and requirements package.

## Verification

Fresh slice-level verification passed: `uv run python scripts/verify_m034_rd_consistency_audit.py --package-dir .gsd/milestones/M034-kuei9y/decision-package --requirements .gsd/REQUIREMENTS.md --decisions .gsd/DECISIONS.md && uv run python scripts/verify_m034_adr_template_and_north_star.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run python scripts/verify_m034_formal_adr_package.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run python scripts/verify_m034_prd_requirements.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run ruff check scripts/verify_m034_rd_consistency_audit.py scripts/verify_m034_adr_template_and_north_star.py scripts/verify_m034_formal_adr_package.py scripts/verify_m034_prd_requirements.py` returned exit 0.

## Requirements Advanced

- R054 — Functional requirements define persisted jobs, lazy recomputation, dependency readiness, and status inspection.
- R055 — Functional/NFR docs require typed failures, retry state, backend/cache health visibility, and observability.
- R056 — Safety requirements preserve sidecar outputs as candidate evidence and graph flags false.
- R059 — PRD/NFRs require GraphDB portability and defer final GraphDB selection.
- R060 — PRD/requirements frame the system as a universal KB with scientific-paper first-domain adapters.
- R061 — S04 docs incorporate S01 audit clarifications into product/requirement language.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

The first combined drafting check failed because Functional Requirements did not include explicit `graph_import_allowed=false`; SFR-004 was added to make safety defaults explicit.

## Known Limitations

S04 requirements are documentation contracts only; implementation and detailed contract schemas are deferred to S05 and future milestones.

## Follow-ups

S05 must turn PRD/requirements into conceptual contracts and invariants, especially `KnowledgeSubstratePort`, job/artifact/failure records, review packets, and safety flags.

## Files Created/Modified

- `.gsd/milestones/M034-kuei9y/decision-package/PRD.md` — Product requirements document for universal evidence orchestration.
- `.gsd/milestones/M034-kuei9y/decision-package/FUNCTIONAL-REQUIREMENTS.md` — Functional requirements split by generic, paper-specific, and safety scope.
- `.gsd/milestones/M034-kuei9y/decision-package/NON-FUNCTIONAL-REQUIREMENTS.md` — Non-functional requirements for locality, reproducibility, redaction, observability, resumability, GraphDB portability, and fail-closed defaults.
- `scripts/verify_m034_prd_requirements.py` — Verifier for PRD and requirements package.
