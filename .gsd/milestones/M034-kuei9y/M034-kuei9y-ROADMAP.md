# M034-kuei9y: Universal Knowledge Base ADR Package

**Vision:** Convert M033 and the follow-up discussion into a strict architecture decision package rooted in the broader project direction: a local-first universal knowledge base with scientific-paper knowledge as the primary current domain and validation path. The package starts with a full audit of existing GSD requirements and decisions for contradictions, then uses a Mermaid-assisted enhanced ADR template to document the universal KB north star, deferred GraphDB selection, sidecar/evidence orchestration boundaries, contracts, roadmap gates, and correction or discussion paths for any conflicts.

## Success Criteria

- All existing GSD Rxxx and Dxxx records are audited first for consistency with the proposed universal knowledge-base framing, GraphDB deferral, sidecar pipeline boundaries, and safety invariants.
- Conflict findings are categorized before ADR drafting as consistent, historical-scope-only, needs clarification, superseded-by-new-ADR, or conflict-needs-user-decision.
- A Mermaid-assisted enhanced ADR template is documented and used consistently: prose and tables remain authoritative, Mermaid diagrams are bounded and readability-driven.
- A north-star ADR frames daily-archive as a local-first universal knowledge base with scientific articles as the primary first domain.
- ADR set captures accepted, rejected, deferred, and open choices from M033 without prematurely locking GraphDB choice, parser adoption, or agentic orchestration.
- GraphDB selection is explicitly deferred with evaluation criteria for LadybugDB, FalkorDB, HelixDB, and other candidates.
- PRD defines bounded product scope for lazy async sidecar orchestration as a generic evidence-pipeline capability with paper-specific first adapters.
- Functional and non-functional requirements are enumerated with validation criteria and tied back to universal knowledge-base evidence chains.
- Contracts are drafted for generic records/jobs/artifacts and paper-specific sidecar outputs, status transitions, failures, review packets, safety flags, and graph-readiness handoff.
- Roadmap includes explicit architecture brainstorm and decision gates before implementation slices.
- Closeout records which conflicts were resolved, deferred, or require user discussion.

## Slices

- [x] **S01: R and D Conflict Audit First** `risk:high` `depends:[]`
  > After this: After this, all existing GSD requirements and decisions have been checked before ADR drafting, with conflicts routed to correction or user discussion.

- [x] **S02: ADR Template and Universal KB North Star** `risk:high` `depends:[S01]`
  > After this: After this, the package has a strict Mermaid-assisted ADR template and a north-star ADR grounded in the universal knowledge-base mission.

- [x] **S03: Formal ADR Package and GraphDB Deferral** `risk:high` `depends:[S01,S02]`
  > After this: After this, accepted architecture choices, deferred GraphDB selection, and rejection boundaries are expressed as formal Mermaid-assisted ADRs.

- [x] **S04: PRD and Requirement Package** `risk:medium` `depends:[S03]`
  > After this: After this, the next implementation track has product scope, users, workflows, goals, non-goals, and functional requirements tied to the universal evidence pipeline.

- [x] **S05: Contracts and Invariants** `risk:high` `depends:[S04]`
  > After this: After this, future implementation has draft contracts for generic knowledge records, jobs, artifacts, sidecars, failure taxonomy, review packets, graph-readiness handoff, GraphDB portability, and safety invariants.

- [x] **S06: Roadmap Gates and Conflict Resolution Plan** `risk:medium` `depends:[S05]`
  > After this: After this, the implementation roadmap has mandatory architecture gates and every remaining R/D conflict has a correction, deferral, or user-discussion path.

- [x] **S07: Decision Package Closeout and Handoff** `risk:low` `depends:[S06]`
  > After this: After this, the package is ready for a stricter tool or next workflow to consume without losing project context.

## Boundary Map

## In Scope
- Full GSD requirement/decision consistency audit across Rxxx and Dxxx records before ADR drafting.
- Mermaid-assisted enhanced ADR template for human and LLM readability.
- North-star ADR for a local-first universal knowledge base with scientific articles as the primary first domain.
- ADRs for post-M033 architecture decisions.
- Deferred GraphDB selection ADR and comparison criteria for LadybugDB, FalkorDB, HelixDB, and other candidates.
- PRD for lazy async sidecar/evidence pipeline scope as a generic capability with paper-specific first adapters.
- Functional and non-functional requirements.
- Contract inventory and draft schemas at conceptual level.
- Roadmap with architecture decision gates.
- Open question register and documentation completeness checklist.

## Out of Scope
- Implementing the queue or sidecar workers.
- Running GROBID, OpenDataLoader, Adaptix, quant-mind, or GraphDB probes.
- Selecting a final production GraphDB.
- Production parser adoption.
- LadybugDB/FalkorDB/HelixDB writes or graph import.
- Agentic orchestration runtime.

## Hard Safety Boundary
Parser sidecar outputs remain candidate evidence only. Graph database selection remains open. All graph/import safety flags remain false unless a later explicitly authorized milestone changes them with evidence. Existing GSD decisions are append-only: contradictions are resolved by new superseding decisions or explicit requirement updates, not silent historical rewrites.
