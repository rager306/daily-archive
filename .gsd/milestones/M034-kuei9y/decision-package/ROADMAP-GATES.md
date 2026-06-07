# M034 Roadmap Gates

No implementation milestone should start coding before these gates are resolved or explicitly deferred.

| Gate | Question | Options | Decision Criteria | Required Artifact Before Coding |
|---|---|---|---|---|
| Universal KB Scope Gate | What is generic versus paper-specific in the prototype? | generic core first / paper adapter first / mixed | Must preserve ADR-000 and R060. | Scope ADR or milestone context section. |
| GraphDB Evaluation Gate | How will LadybugDB, FalkorDB, HelixDB, and others be compared? | defer / run comparison / choose candidate | License, locality, performance, graph-vector, portability. | GraphDB comparison matrix plan; no final selection yet. |
| State Model Gate | What persisted records exist? | job-centric / artifact-centric / hybrid | Resume, stale detection, observability. | State model contract. |
| Queue Semantics Gate | How are jobs claimed/retried? | SQLite / filesystem manifest / hybrid | Lease, retry_after, dead-letter, crash recovery. | Queue semantics ADR/prototype plan. |
| Artifact Dependency Graph Gate | What makes downstream stale or ready? | source-hash only / artifact graph / full dependency graph | Lazy recompute correctness. | Dependency graph spec. |
| Failure Taxonomy Gate | Which failures retry or block? | minimal / full taxonomy | Must match FAILURE-TAXONOMY.md. | Failure code matrix. |
| Sidecar Lifecycle Gate | How are GROBID/OpenDataLoader/Adaptix started and checked? | per-worker / managed service / manual | Backend/cache health and bounded concurrency. | Sidecar lifecycle runbook. |
| Review Boundary Gate | What constitutes completed review? | deterministic / human / LLM-assisted / hybrid | Must not bypass review packet. | Review packet completion contract. |
| Graph-readiness Handoff Gate | What can readiness handoff claim? | no-write only / import candidate / import authorized | Must preserve safety flags false unless future authorization. | Handoff contract and verifier. |
| Agent Boundary Gate | Can agents participate? | no / helper-only / orchestrator | Must satisfy ADR-006. | Agent helper ADR if not no. |

## Implementation Ordering

1. Resolve gates as design artifacts.
2. Prototype persisted state and queue without sidecar runtime.
3. Add one paper-domain sidecar worker in no-write mode.
4. Add stale/retry/resume verification.
5. Add review packet and readiness handoff verification.
6. Defer GraphDB writes and agentic orchestration.

## Non-Authorization

This roadmap does not authorize final GraphDB selection, production graph import, GraphDB writes, parser-as-truth, or agentic orchestration.
