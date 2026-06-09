# S02: ADR Template and Universal KB North Star

**Goal:** Finalize the Mermaid-assisted enhanced ADR template as the binding format for M034 and draft the universal-KB north-star ADR grounded in S01 audit findings, M033 conclusions, and the user's corrected architecture direction.
**Demo:** After this, the package has a strict Mermaid-assisted ADR template and a north-star ADR grounded in the universal knowledge-base mission.

## Must-Haves

- Template includes mandatory sections for context, decision, applies-to, R/D impact, options, tradeoffs, consequences, safety non-authorization, contract impact, validation evidence, open questions, follow-ups, supersession, and LLM reading notes.
- Template states prose and tables are authoritative; Mermaid diagrams are optional and bounded for context maps, safety gates, state transitions, option comparisons, and contract relationships.
- Template limits diagram use so readability is preserved.
- North-star ADR states the project purpose: local-first universal knowledge base built from durable, traceable evidence chains.
- North-star ADR identifies scientific articles as the primary first domain, not the only possible content type.
- North-star ADR references R024, R027, R029, R040, R050, R054-R061, D067, M033 conclusions, and S01 audit findings.

## Proof Level

- This slice proves: Template checklist plus document inspection against PROJECT.md, REQUIREMENTS.md, DECISIONS.md, M033 safety invariants, and S01 audit findings.

## Integration Closure

Provides the architecture frame and ADR format that all later ADRs, PRD sections, contracts, and roadmap gates must cite.

## Verification

- Defines evidence-chain visibility, fail-closed state transitions, and readable decision surfaces as project-level obligations.

## Tasks

- [x] **T01: Finalized the ADR template convention and created the M034 ADR index.** `est:small`
  Review the physical `ADR-TEMPLATE.md`, ensure it includes the full Mermaid-assisted enhanced structure and readability rules, and create an ADR index stub that records the template requirement and planned ADR set.
  - Files: `.gsd/milestones/M034-kuei9y/decision-package/ADR-TEMPLATE.md`, `.gsd/milestones/M034-kuei9y/decision-package/ADR-INDEX.md`
  - Verify: Check that the template includes required sections 0-14, Mermaid readability rules, special diagram blocks, and LLM Reading Notes; check ADR index references the template path.

- [x] **T02: Drafted ADR-000 as the binding universal-KB north-star decision.** `est:medium`
  Create the north-star ADR using the template. It must define daily-archive as a local-first universal knowledge base, keep scientific articles as the first proving domain, separate generic primitives from paper-specific adapters, preserve evidence-chain promotion, defer GraphDB selection, and explicitly state safety non-authorizations.
  - Files: `.gsd/milestones/M034-kuei9y/decision-package/ADR-000-universal-kb-north-star.md`
  - Verify: Check ADR-000 contains all required template sections, at least one Mermaid context or evidence-chain diagram, R/D impact tables, safety non-authorization, and LLM Reading Notes.

- [x] **T03: Added and passed the verifier for the M034 ADR template and north-star package.** `est:small`
  Implement and run a verifier for the ADR template/index/north-star package, checking template sections, ADR-000 sections, safety markers, R/D references, Mermaid readability constraints, and S01 audit consumption.
  - Files: `scripts/verify_m034_adr_template_and_north_star.py`
  - Verify: `uv run python scripts/verify_m034_adr_template_and_north_star.py --package-dir .gsd/milestones/M034-kuei9y/decision-package && uv run ruff check scripts/verify_m034_adr_template_and_north_star.py`

## Files Likely Touched

- .gsd/milestones/M034-kuei9y/decision-package/ADR-TEMPLATE.md
- .gsd/milestones/M034-kuei9y/decision-package/ADR-INDEX.md
- .gsd/milestones/M034-kuei9y/decision-package/ADR-000-universal-kb-north-star.md
- scripts/verify_m034_adr_template_and_north_star.py
