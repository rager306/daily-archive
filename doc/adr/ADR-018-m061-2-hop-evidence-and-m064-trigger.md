# ADR-018: M061 2-hop Evidence and M064 Trigger Evaluation

**Status:** Accepted (binding)  
**Date:** 2026-06-13  
**Deciders:** agent  
**Milestone:** M064-wqfgfa S03  
**Scope:** m061-2-hop-bfs / pipeline-queue-trigger / arxiv-rate-limit / m3-judge / graph-diagnostics  
**Binding Level:** binding supplement to ADR-010, ADR-013, ADR-014, ADR-016, ADR-017  
**Revisable:** yes, after M062 production hardening and M063 GraphDB selection are complete and fresh evidence shows synchronous execution is no longer sufficient

## 0. One-line Decision

> M061 2-hop BFS is complete at 5-anchor scale, the 5-layer diagnostic graph is validated, and the M064 queue trigger evaluation is **CONFIRM DEFER** because synchronous execution remains sufficient under ADR-017.

## 1. Context

M061 executed a 2-hop BFS pilot and scale-out across five anchors from the M056 pattern. The run combined real arXiv acquisition, manifest-driven processing, figure QA diagnostics through M3 evidence, and a 5-layer graph validation.

ADR-017 says the pipeline queue is deferred until the pipeline is end-to-end complete and evidence shows async queue execution is needed. M061 is one required evidence milestone, not by itself permission to build queue infrastructure.

## 2. Decision

We accept the M061 evidence package and bind the following decisions:

1. The 5-layer diagnostic graph is validated for M061 evidence use.
2. The M064 trigger condition is not met; the queue remains deferred.
3. Synchronous execution remains the required execution mode for this pipeline phase.
4. M062 and M063 stay ahead of any M064 implementation work.

Decision outcome: **CONFIRM DEFER M064**.

## 3. Scope and Non-Scope

In scope:

- M061 5-anchor 2-hop BFS evidence synthesis.
- arXiv pacing and HTTP 429 evaluation.
- M3 judge diagnostic integration evidence.
- 5-layer graph validation evidence.
- M064 trigger decision under ADR-017.

Out of scope:

- Building queue infrastructure.
- Enabling GraphDB writes.
- Promoting diagnostic facts into production.
- Changing M062 or M063 milestone scope.

## 4. Requirements and Decisions Impacted

| Item | Impact | Result |
|---|---|---|
| ADR-010 | Extends 2-hop BFS evidence with 5 anchors | consistent |
| ADR-013 | Uses manifest-driven ingest artifacts as evidence sources | consistent |
| ADR-014 | Confirms M3 diagnostic judge integration at M061 scale | consistent |
| ADR-016 | Uses NetworkX + igraph graph-library posture for diagnostic graph work | consistent |
| ADR-017 | Evaluates M064 trigger and confirms deferral | binding |
| M045 trajectory | No high-severity drift introduced | on_track |
| M044 guardrail | Architecture guardrail remains satisfied | ok |

## 5. Options Considered

| Option | Description | Decision |
|---|---|---|
| Build M064 queue now | Start async scheduler, per-article DAG, leases, and multi-worker execution immediately after M061 | rejected |
| Confirm defer | Keep sync execution until M062 and M063 complete and queue need is evidenced | accepted |
| Cancel queue permanently | Declare queue infrastructure never needed | rejected |

## 6. Trade-off Analysis

Confirming deferral avoids building infrastructure ahead of evidence. M061 processed 150 real papers at 7.11 papers/min with 0 HTTP 429s, which is enough for current validation work.

The trade-off is that future larger runs may still need queue execution. ADR-017 remains revisable when M062, M063, and fresh scale evidence demonstrate a concrete async requirement.

## 7. Consequences

- M061 closes as evidence-complete.
- M064 implementation remains deferred.
- Future agents should not treat M061 success as authorization to build queue infrastructure.
- M062 and M063 remain the next evidence milestones.
- Queue design discussions must cite this ADR and ADR-017.

## 8. Safety and Non-Authorization

External network is disabled by default. LLM calls are disabled by default. Graph writes is not authorized. Production import is not authorized. Fact promotion is not authorized.

M061 used scoped overrides only for real arXiv acquisition and diagnostic M3 evidence. Those overrides do not change defaults and do not authorize queue execution.

## 9. Contract Impact

The processing contract remains manifest-driven and synchronous. Evidence files are:

- `artifacts/m061-2hop/REPORT.md`
- `artifacts/m061-2hop/m061-summary.json`
- `artifacts/m061-2hop/m061-decision.md`
- `artifacts/m061-2hop/5-anchor-5-layer-graph-manifest.json`

The required host reference is `127.0.0.1`; no alternate loopback hostname should be introduced into source or markdown for this milestone.

## 10. Validation / Evidence Required

| Evidence | Required result | Observed |
|---|---|---|
| Anchors | 5 complete anchors | 5 |
| arXiv requests | 0 HTTP 429s | 0 |
| Throughput | >= 1 paper/min | 7.11 |
| M3 judge | >= 80% success | 100.0% |
| Citation graph | 2662 nodes / 8911 edges | 2662 / 8911 |
| 5-layer graph | structurally valid | true |

## 11. Open Questions

- What M062 production hardening evidence will most strongly indicate pipeline bottlenecks?
- Which GraphDB substrate will M063 choose for durable graph persistence?
- At what larger corpus size does sync execution become operationally insufficient?

None of these questions block M061 closeout.

## 12. Follow-up Actions

1. Continue with M062 fd production hardening.
2. Continue with M063 GraphDB selection.
3. Re-evaluate M064 only after ADR-017 revisability conditions are satisfied.
4. Keep `.codebase-memory` synchronized with this ADR for governance readback.

## 13. Supersedes / Superseded By

This ADR does not supersede ADR-017. It is a binding trigger-evaluation supplement to ADR-017.

Superseded by: none.

## 14. LLM Reading Notes

- If asked whether M061 authorizes queue infrastructure, answer: no; ADR-018 confirms defer.
- If asked whether M061 validated the 5-layer graph, answer: yes, for diagnostic evidence use.
- If asked whether GraphDB writes are allowed, answer: graph writes is not authorized.
- If asked whether production import is allowed, answer: production import is not authorized.
- If asked whether external network or LLM calls are enabled by default, answer: external network is disabled by default and LLM calls are disabled by default.
- Use `artifacts/m061-2hop/REPORT.md` and `artifacts/m061-2hop/m061-summary.json` as the concise evidence packet.
