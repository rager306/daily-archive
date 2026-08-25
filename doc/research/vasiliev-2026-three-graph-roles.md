# Research note: three graph roles (Vasiliev 2026)

- Source: `data/sources/blog/vasiliev2026knowledge-graph-memory-graph/`
- Canonical URL: see identity.json (not hardcoded here)
- Citation key: `vasiliev2026knowledge-graph-memory-graph`
- Captured: 2026-08-25
- Status: architecture source, **not** graph truth (D127)

## Claim

Knowledge graph, memory graph and context graph are **roles** on shared
identity/time/provenance, not three platforms to buy. Agent memory is
broader than a memory graph (session, events, vectors, skills). A memory
graph is justified when relationships, time and provenance change retrieval
or action.

## Alignment

Already in-repo: ADR-040 three-tier (Samyama / RuVector / RVF), D127
candidate-vs-governed, ADR-053 Layer H ContextBundle, ADR-055 ACAP,
M454 critic Reject as typed negative knowledge, CVF as source event record.

## Gaps the article names (product, not corpus wave)

1. Memory write-boundary must quarantine unreviewed claims.
2. Retrieval must honour rejection (Negative Semantic Knowledge) — closed
   M459 (`admit_retrieval` paper-scoped; `agent_failure:negative_blocked`).
3. Context freeze of source + contract versions — closed M458 (`freeze_id`).

Full HTML is the captured source. This note is a mapping, not a substitute.
