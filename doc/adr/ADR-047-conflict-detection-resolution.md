# ADR-047: Conflict Detection and Resolution

**Status:** Proposed
**Date:** 2026-07-24
**Deciders:** collaborative
**Related:** ADR-040 (Samyama store), ADR-042 (Claim), ADR-044 (healing), ADR-045 (validator), ADR-046 (bi-temporal), [Semantica conflicts module](https://github.com/semantica-agi/semantica/blob/main/semantica/conflicts/)

## Context

daily-archive currently silently overwrites or deduplicates when two sources
disagree:

- Two papers citing the same arxiv_id with different titles — first write wins.
- Two extractions of the same entity label with different entity_types — last
  write wins.
- A retraction (paper A says claim B is wrong) — no representation in the graph.
- MetricObservation from paper A=0.92, paper B=0.87 for the same method/dataset
  — both stored, no link.

The existing `SUPERSEDES` edge (ADR-044 healing) handles merge-driven
deprecation, not factual disagreement. `REFUTES` (process plane) exists as an
edge constant but is not materialized in pipeline. There is no `Conflict`
node, no detection logic, no resolution strategy registry.

For a scientific KG this is a correctness gap: contradictions between papers
are themselves first-class scientific facts. Silently dropping them destroys
information that downstream users (researchers, meta-analysts, auditors)
specifically want to see.

## Decision

Introduce a `Conflict` node type and a `ConflictDetector` use case, modelled
on Semantica's conflicts module but adapted to scientific KG semantics.

### Conflict node (new)

| Property           | Type     | Required | Description                                                       |
|--------------------|----------|----------|-------------------------------------------------------------------|
| `vid`              | String   | yes      | `vid:conflict:{hash}` deterministic from participants + field.    |
| `kind`             | String   | yes      | `factual` / `typological` / `temporal` / `metric`.                |
| `field`            | String   | yes      | Property name in disagreement (e.g. `title`, `entity_type`, `value`). |
| `status`           | String   | yes      | `detected` / `resolved` / `escalated` / `archived`.               |
| `severity`         | String   | yes      | `low` / `medium` / `high` (drives resolution ordering).           |
| `resolution_strategy` | String |          | One of the strategies below (set when status=resolved).           |
| `resolution_value` | String   |          | Winning value when status=resolved.                               |
| `detected_at`      | DateTime | yes      | Transaction time of detection (bi-temporal, ADR-046).             |
| `resolved_at`      | DateTime |          | Transaction time of resolution.                                   |
| + invariants       |          | yes      | vid, retrieval_eligible, import_eligible, schema_version.         |

### Conflict edges (new)

- `CONFLICTS_OVER` — `Claim/Entity/Reference/MetricObservation → Conflict`.
  Hyperedge-style: 2+ participants per conflict.
- `RESOLVED_BY` — `Conflict → Source` or `Conflict → Decision`. Records
  which source or authority resolved the conflict.
- Reuses existing `REFUTES` (Claim → Claim) for direct refutation edges
  without a Conflict node (lighter weight for bilateral disagreement).

### Conflict kinds

| Kind          | Trigger                                                            | Example                                                          |
|---------------|--------------------------------------------------------------------|------------------------------------------------------------------|
| `factual`     | Same entity, same property, different values across sources.       | Paper A says method X is from 2019, paper B says 2018.           |
| `typological` | Same surface form, different entity_type.                          | "Transformer" as Method vs Model.                                |
| `temporal`    | Same fact, different valid_from.                                   | Two papers claim priority for the same method.                   |
| `metric`      | Same method × dataset, different observed values.                  | accuracy 0.92 vs 0.87 on GLUE.                                   |

### Resolution strategies (pluggable)

Adopted directly from Semantica's `ResolutionStrategy` enum, with
scientific-KG adjustments:

| Strategy              | When to use                                                    |
|-----------------------|----------------------------------------------------------------|
| `voting`              | 3+ sources, majority wins. Tie → fall through to credibility.  |
| `credibility_weighted`| Source.reliability_tier weights votes. Tier 1 > tier 2 > 3.    |
| `most_recent`         | valid_from decides. For "current state of knowledge" queries.  |
| `first_seen`          | recorded_at decides. For "original claim" preservation.        |
| `highest_confidence`  | Extraction confidence decides. For LLM-extracted facts.        |
| `retain_both`         | No resolution; both values kept, conflict flagged. Default for |
|                       | metric conflicts (different measurement protocols).            |
| `manual_review`       | Escalate to human. Sets status=escalated.                      |
| `expert_review`       | Escalate to domain expert queue. Future (ADR-048).             |

Default strategy per kind, overridable per conflict via CLI/config:

```toml
[conflicts.default_strategy]
factual = "credibility_weighted"
typological = "retain_both"
temporal = "retain_both"
metric = "retain_both"
```

### Pipeline integration

New use case `crates/da-application/src/conflicts.rs`:

```rust
pub struct ConflictDetectionUseCase {
    graph_store: Box<dyn DirectGraphStore>,
    detector: ConflictDetector,
    resolver: ConflictResolver,
}

impl ConflictDetectionUseCase {
    pub async fn scan_new_facts(&self, since: DateTime) -> Result<ConflictScanResult>;
    pub async fn resolve(&self, conflict_id: &str, strategy: ResolutionStrategy) -> Result<ResolutionResult>;
    pub async fn escalate(&self, conflict_id: &str, reason: &str) -> Result<()>;
}
```

Runs after extraction (detects factual/typological conflicts on newly-written
nodes) and after enrich (detects temporal conflicts when OpenAlex refines
metadata).

### Cross-ADR alignment

- **ADR-042**: Claim can be a conflict participant. Conflict replaces the
  ad-hoc "later write wins" pattern.
- **ADR-044**: healing.rs `SUPERSEDES` continues for merge-driven
  deprecation; Conflict is for disagreement, not deduplication.
- **ADR-045**: validator gains a `conflict-consistency` rule — a node with
  active conflict must not have `retrieval_eligible=true` unless the
  conflict is `resolved` or `retain_both`.
- **ADR-046**: all Conflict timestamps are bi-temporal.
- **ADR-048**: Decision records can resolve conflicts (RESOLVED_BY edge).

## Alternatives considered

1. **Reuse healing.rs for conflicts.** Rejected — healing assumes one node
   is wrong and should be silenced. Conflicts assume both nodes may be right
   under different valid-time windows.
2. **Edge-only model (no Conflict node).** Rejected — hyperedge with 2+
   participants and resolution lifecycle needs reification. Pure edge
   `Claim REFUTES Claim` is kept for bilateral cases.
3. **Event-sourced conflict log.** Defer — the Conflict node + bi-temporal
   fields (ADR-046) capture the same audit trail without the query cost.

## Consequences

- New node type `Conflict` (29 → 30 schemas).
- New edges: `CONFLICTS_OVER`, `RESOLVED_BY` (13 → 15 edge constants).
- Conflict detector runs on every ingest batch; adds latency proportional
  to number of new fact-bearing nodes × participants per fact.
- Source.reliability_tier (currently unused) becomes load-bearing for the
  `credibility_weighted` strategy.
- CLI gains `da conflicts list|resolve|escalate` commands.
- Retractions, priority disputes, and metric disagreements become
  first-class queryable facts instead of silent overwrites.

## Open questions

- Should Conflict participate in cross-reference validation (ADR-045 Wave F)?
  Tentative yes — `RESOLVED_BY → Source` is a cross-reference.
- Default conflict detection window: per-batch only, or re-scan full graph
  on each ingest? Defer to performance testing.
