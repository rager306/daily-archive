# M034 ADR Index

Milestone: `M034-kuei9y`  
Package: Universal Knowledge Base ADR Package

## Template Rule

All M034 ADRs must use the physical template:

- `.gsd/milestones/M034-kuei9y/decision-package/ADR-TEMPLATE.md`

Template constraints:

- Prose and tables are authoritative.
- Mermaid diagrams are optional and bounded.
- Diagrams clarify context, safety gates, status transitions, option comparisons, or contract relationships.
- Every ADR must include `LLM Reading Notes`.
- Every ADR must state safety non-authorization where applicable.
- Every ADR must include impacted R/D records or explicitly state why none apply.

## ADR Status Vocabulary

- `Proposed` — drafted but not accepted.
- `Accepted` — binding for future M034 artifacts unless superseded.
- `Deferred` — decision intentionally postponed; ADR may still be binding as a non-lock-in rule.
- `Rejected` — option explicitly rejected for current scope.
- `Superseded` — replaced by a later ADR or GSD decision.

## Planned ADRs

| ADR | Title | Status | Binding Level | Scope | Related R/D | Notes |
|---|---|---|---|---|---|---|
| ADR-000 | Universal KB North Star | Accepted | binding | universal-kb / safety | R024,R027,R029,R040,R050,R054-R061,D065-D067 | Establishes project frame: universal local-first KB, scientific articles as first proving domain. |
| ADR-001 | Scientific Papers as First Domain | Planned | binding | universal-kb / evidence-pipeline | R024,R027,R029,R031,R033,R050,R060 | Clarifies paper-domain requirements as first-domain validation, not the only KB domain. |
| ADR-002 | Defer Final GraphDB Selection | Deferred | binding non-lock-in | graphdb | R019,R056,R059,D012,D061,D065 | Keeps LadybugDB/FalkorDB/HelixDB/other choice open pending comparison. |
| ADR-003 | Durable Lazy Async Evidence Pipeline | Accepted | directional | evidence-pipeline | R054,R055,R057,D063 | Defines durable queue/status/retry/artifact direction before implementation. |
| ADR-004 | Sidecars as Candidate Evidence Producers | Accepted | binding | sidecar / safety | R056,D061,D062,D063 | GROBID/OpenDataLoader/Adaptix produce candidates, not graph-ready truth. |
| ADR-005 | No Direct Extractor to GraphDB Path | Accepted | binding | safety / graphdb | R056,R059,D064 | Blocks parser/LLM/sidecar direct writes to any GraphDB. |
| ADR-006 | Agent Boundary | Accepted | binding | agent-boundary | D036,D064 | Agents may assist later but do not orchestrate or promote graph facts now. |
| ADR-007 | Quant-mind Pattern Source Not Runtime Dependency | Accepted | directional | sidecar / agent-boundary | D062,D063,D064 | Captures useful patterns without adopting runtime dependency. |

## S01 Audit Inputs

The ADR package must consume:

- `r-d-consistency-audit.json`
- `R-D-CONSISTENCY-AUDIT.md`
- `correction-checklist.md`
- `open-conflicts-for-user.md`
- `correction-routes.json`

Current S01 counts:

- Requirements: 61
- Decisions: 67
- Audit records: 128
- Consistent: 35
- Historical scope only: 78
- Needs clarification: 15
- Blocking conflicts needing immediate user decision: 0

## Non-Authorization Reminder

This index does not authorize:

- production graph import;
- final GraphDB selection;
- LadybugDB/FalkorDB/HelixDB writes;
- parser output as graph-ready truth;
- agentic orchestration;
- bypassing validators or review packets.
