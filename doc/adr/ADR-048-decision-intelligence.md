# ADR-048: Decision Intelligence — First-Class Decision Records

**Status:** Proposed
**Date:** 2026-07-24
**Deciders:** collaborative
**Related:** ADR-043 (process plane), ADR-044 (healing decisions), ADR-045 (validator), ADR-046 (bi-temporal), ADR-047 (conflicts), [Semantica context module](https://github.com/semantica-agi/semantica/blob/main/semantica/context/)

## Context

daily-archive's process plane (ADR-043) treats ResearchProblem, Hypothesis,
ExperimentRun, ResultComparison, and FailureEvent as **static extracted
facts**. Once written, they sit in the graph until manually healed. There is
no representation of:

- **Why** an agent or human decided to re-run an experiment.
- **Which prior decisions** influenced a new hypothesis.
- **What was the outcome** of adopting a specific methodological choice.
- **Which policy** (deprecation, promotion, gating) authorized a healing
  action (ADR-044 mentions "reason" string but not as a structured decision).
- **How to find precedent** for a current decision (semantic similarity over
  the decision graph).

ADR-044 healing.rs captures a free-text `reason` and writes
`HealingProvenance`. That is the closest existing thing to a decision record,
but it is procedurally embedded in the healing use case, not queryable as a
graph citizen.

Semantica's Decision Intelligence layer (`semantica.context.Decision`,
`ContextGraph.record_decision`, causal relationship types, policy engine)
shows how first-class decision records unlock "why did the system do that?"
queries that auditors, meta-analysts, and the agent itself need.

## Decision

Introduce a `Decision` node type and causal-link edges, modelled on
Semantica's Decision lifecycle but scoped to scientific KG and agent
operations.

### Decision node (new)

| Property              | Type        | Required | Description                                                          |
|-----------------------|-------------|----------|----------------------------------------------------------------------|
| `vid`                 | String      | yes      | `vid:decision:{uuid}` or deterministic from actor+scenario+ts.       |
| `category`            | String      | yes      | Controlled vocabulary (see below).                                   |
| `scenario`            | String      | yes      | Short description of what was decided.                               |
| `reasoning`           | String      | yes      | Why the decision was made. Free text.                                |
| `outcome`             | String      | yes      | Controlled vocabulary per category (e.g. `approved`, `rejected`).    |
| `confidence`          | Float       | yes      | 0.0–1.0. For LLM-derived decisions, model confidence.                |
| `decision_maker`      | String      | yes      | `agent:{id}` / `human:{id}` / `system:{component}`.                  |
| `policy_id`           | String      |          | Reference to the Policy authorizing this decision (if any).          |
| `valid_from`          | DateTime    | yes      | Bi-temporal (ADR-046).                                               |
| `valid_to`            | DateTime?   |          | Open until superseded.                                               |
| `recorded_at`         | DateTime    | yes      | Bi-temporal.                                                         |
| `superseded_at`       | DateTime?   |          | Bi-temporal.                                                         |
| `reasoning_embedding` | Vector?     |          | For semantic precedent search (Phase 2).                             |
| + invariants          |             | yes      | vid, retrieval_eligible, import_eligible, schema_version.            |

### Decision categories (controlled vocabulary)

Scoped to daily-archive use cases (not Semantica's enterprise lending/medical
categories):

| Category               | Scenario examples                                                    |
|------------------------|----------------------------------------------------------------------|
| `healing_action`       | silence / correct / merge entity (replaces ADR-044 reason string).   |
| `conflict_resolution`  | pick winning value in a Conflict (ADR-047).                          |
| `promotion`            | import_eligible true→false (D127 gate).                              |
| `schema_migration`     | apply ADR-044 migration plan.                                        |
| `extraction_override`  | human corrects LLM-extracted entity/relation.                        |
| `hypothesis_tested`    | ResultComparison outcome (Confirmed/Refuted/Mixed).                  |
| `retraction_recorded`  | paper retracted; supersede linked Claims.                            |
| `deprecation`          | concept cluster deprecated; retrieval_eligible=false.                |
| `policy_applied`       | PolicyEngine evaluated a rule (future).                              |

Categories are data-driven (`data/decision_categories.yaml`) per project
convention "не хардкодим" — logic in Rust, vocabularies in YAML.

### Causal edges (new)

Adopt Semantica's three causal primitives, scoped to scientific use:

- `CAUSED` — Decision A caused Decision B. Strong.
- `INFLUENCED` — Decision A was a factor in Decision B. Weak.
- `PRECEDENT_FOR` — Decision A is a precedent cited by Decision B.

Plus domain-specific:
- `AUTHORITY_FOR` — Policy → Decision (policy authorized the decision).
- `TRIGGERED_BY` — Decision ← Conflict | HealingAction | ExtractionOverride.

### Policy node (future, Phase 2)

Reserved for a `Policy` node type with SHACL-like rules. Phase 1 records
`policy_id` as an opaque string reference; Phase 2 materializes Policy nodes
when the rule engine (ADR-049 candidate) lands.

### Decision lifecycle

```
1. record_decision()       — write Decision node + invariants.
2. add_causal_relationship() — link to upstream causes / downstream effects.
3. find_similar_decisions()  — semantic precedent search over reasoning_embedding.
4. trace_decision_chain()    — walk CAUSED/INFLUENCED ancestry to root.
5. analyze_decision_impact() — walk forward to leaves (decisions this one caused).
6. check_decision_rules()    — policy gate (Phase 2).
7. export_prov_o()           — W3C PROV-O audit trail for regulators/reviewers.
```

### Pipeline integration

New module `crates/da-application/src/decisions.rs`:

```rust
pub struct DecisionRecorder {
    graph_store: Box<dyn DirectGraphStore>,
}

impl DecisionRecorder {
    pub async fn record(&self, req: DecisionRequest) -> Result<DecisionRecord>;
    pub async fn add_cause(&self, cause: &str, effect: &str, rel: CausalRelation) -> Result<()>;
    pub async fn find_precedents(&self, scenario: &str, k: usize) -> Result<Vec<DecisionRecord>>;
    pub async fn trace_chain(&self, decision_id: &str) -> Result<CausalChain>;
    pub async fn analyze_impact(&self, decision_id: &str) -> Result<ImpactMap>;
}
```

Wiring points:
- **healing.rs** — every healing action wraps itself in a Decision of
  category `healing_action`. Replaces free-text `reason` with structured
  `scenario`/`reasoning`/`outcome`.
- **conflicts.rs (ADR-047)** — every resolution writes a Decision of
  category `conflict_resolution` and links via `TRIGGERED_BY`.
- **extraction.rs** — human-in-the-loop override path writes
  `extraction_override` Decision linked to the corrected Entity.
- **scheduler.rs** — retry / fail / escalate decisions are recorded.

### Cross-ADR alignment

- **ADR-042**: ResultComparison (Claim subtype) outcome maps to
  `hypothesis_tested` Decision category. The Decision captures the act of
  declaring a result confirmed/refuted; the ResultComparison node captures
  the measurements.
- **ADR-043**: every process-plane state change (Hypothesis status update,
  ExperimentRun completion) emits a Decision. Process plane gains
  lifecycle instead of being static.
- **ADR-044**: healing provenance becomes a typed Decision record instead
  of a free-text `reason` field. Migration plans reference Decisions.
- **ADR-046**: Decision is fully bi-temporal.
- **ADR-047**: Decisions resolve Conflicts.

### Phase 1 scope

This ADR defines Phase 1:
- Decision node + causal edges.
- DecisionRecorder use case.
- healing.rs migration to Decision records (structurally — data flows to
  both old `reason` field and new Decision node during transition).
- `da decisions record|trace|precedents` CLI commands.

### Phase 2 (out of scope, future ADR)

- Policy node + rule engine (SHACL-style).
- `check_decision_rules()` policy gate.
- `reasoning_embedding` populated by an embedding model.
- W3C PROV-O export command.
- Cross-agent decision sharing (multi-agent context).

## Alternatives considered

1. **Keep healing.rs `reason` string.** Rejected — unqueryable, no causal
   chain, no precedent search, no policy gate.
2. **Reuse Claim node for decisions.** Rejected — Claim is a proposition
   extracted from source text. Decision is an act by an actor. Conflating
   them mixes epistemic levels.
3. **Event-sourced decision log outside the graph.** Rejected — loses graph
   traversal as the query primitive; audit trail should be one query away.

## Consequences

- New node type `Decision` (30 → 31 schemas after ADR-047 Conflict).
- New edges: `CAUSED`, `INFLUENCED`, `PRECEDENT_FOR`, `AUTHORITY_FOR`,
  `TRIGGERED_BY` (5 new edge constants).
- `decision_categories.yaml` config file added (logic stays in Rust).
- healing.rs writes both legacy `reason` field and new Decision node
  during the transition window.
- Pipeline gains 1 extra write per healing action / conflict resolution /
  human override. Negligible throughput impact.
- "Why did the system do that?" becomes a `da decisions trace <id>` query
  instead of `git log` + grep across `.gsd/` files.

## Open questions

- Should `decision_maker` be a graph node (Agent/Person) or stay a string
  prefix? Phase 1: string. Phase 2: materialize when agent identity module
  lands.
- Causal edge transitivity: is `CAUSED` transitive? Tentative yes (causal
  chain); `INFLUENCED` non-transitive.
- Should Decisions participate in validator's cross-reference check?
  Tentative yes for `policy_id` once Policy node exists.
