# ADR-046: BiTemporal Fact Model — Valid Time + Transaction Time

**Status:** Proposed
**Date:** 2026-07-24
**Deciders:** collaborative
**Related:** ADR-040 (Samyama store), ADR-042 (Claim/EvidenceBundle), ADR-043 (process plane), ADR-044 (schema lifecycle), [Semantica BiTemporalFact](https://github.com/semantica-agi/semantica/blob/main/semantica/kg/temporal_model.py)

## Context

Current temporal model on `Claim`, `EvidenceBundle`, and process-plane nodes
carries a single `valid_from` field (publication timestamp). This collapses two
distinct time dimensions that regulators, auditors, and reproducibility reviews
treat separately:

1. **Valid time** — when the fact was true in the real world (paper publication
   date, experiment run date, claim retraction date).
2. **Transaction time** — when our system learned about, wrote, or superseded
   the fact (extraction timestamp, re-extraction after PDF re-parse, manual
   correction).

Without the split, daily-archive cannot answer:

- "What did we know about method X on 2024-03-15?" (point-in-time read)
- "When did we first extract claim Y, and when was it superseded?" (audit trail)
- "Has this fact been retracted by the source but not yet propagated?" (staleness)
- "Was this observation taken from the v1 or v2 preprint?" (version disambiguation)

The current `valid_from` field on every node answers the first dimension weakly
and the second not at all. ADR-044 (schema lifecycle) addresses schema-version
migration; this ADR addresses fact-level temporality.

## Decision

Adopt a **BiTemporal Fact wrapper** modelled on SQL:2011 temporal tables and
Semantica's `BiTemporalFact`. Every fact-bearing node (Claim, EvidenceBundle,
MetricObservation, ResultComparison, and future Decision) carries four
temporal fields:

| Field           | Type        | Meaning                                                    |
|-----------------|-------------|------------------------------------------------------------|
| `valid_from`    | DateTime    | When the fact became true in the real world.               |
| `valid_to`      | DateTime?   | When the fact stopped being true (`OPEN` = still true).    |
| `recorded_at`   | DateTime    | When our system first wrote this fact.                     |
| `superseded_at` | DateTime?   | When our system stopped using this fact (`OPEN` = current).|

Two fields per axis. Together they answer both time-travel questions.

### Valid time vs transaction time

- **Valid time** (valid_from, valid_to) tracks the source's view. A paper
  published 2023-06-01 claiming "method X achieves 92% accuracy" has
  valid_from=2023-06-01. If the paper is retracted on 2024-09-15, the
  claim's valid_to=2024-09-15. A new claim with valid_from=2024-09-15
  records "method X accuracy disputed."
- **Transaction time** (recorded_at, superseded_at) tracks our system's view.
  We extracted the original claim on 2024-01-10
  (recorded_at=2024-01-10, superseded_at=OPEN). On 2024-09-20 we re-ingest
  the retracted paper; the old Claim gets superseded_at=2024-09-20, and a
  new Claim with recorded_at=2024-09-20 and valid_from=2024-09-15 links
  to the retraction.

### OPEN sentinel

`valid_to` and `superseded_at` may be either a DateTime or the sentinel
`"OPEN"` (Semantica's `TemporalBound.OPEN`). The string sentinel avoids the
classic "0 means null" anti-pattern and is serializable in Samyama's
schemaless store.

### What is a "fact-bearing node"?

Not every node needs bi-temporal fields. Publication-plane metadata nodes
(Paper, Section, Reference, Author, Institution) describe documents and are
effectively immutable once written — a single `valid_from` (publication date)
suffices. Fact-bearing nodes are those whose truth value can change:

- `Claim` — propositional statement extracted from text.
- `EvidenceBundle` — co-occurring entities supporting a claim.
- `MetricObservation` — measured value, may be retracted or corrected.
- `ResultComparison` — comparative statement, may be refuted.
- `Decision` (future, ADR-048) — agent/system decision with lifecycle.
- `ResearchProblem`, `Hypothesis`, `ResearchIdea` — status changes over time.

### Backward compatibility

`Claim` already has `valid_time` (single field). Migration path:

1. Phase 1 (this ADR): add `valid_to`, `recorded_at`, `superseded_at` as
   **optional** fields on fact-bearing schemas. Existing writes populate
   `recorded_at = now()` at create time and leave `valid_to = OPEN`,
   `superseded_at = OPEN`.
2. Phase 2: validator warns when a fact-bearing node is written without
   `recorded_at`. Existing nodes back-filled via migration job.
3. Phase 3: validator fails when `recorded_at` missing on new writes.

### Temporal query helpers

New helpers in `crates/da-domain/src/temporal.rs` (new module):

```rust
pub fn is_active_at(node_snapshot: &PropertySnapshot, at: DateTime) -> bool;
pub fn was_known_at(node_snapshot: &PropertySnapshot, at: DateTime) -> bool;
pub fn supersede(node_id: u64, at: DateTime, store: &dyn DirectGraphStore)
    -> impl Future<Output = Result<()>>;
```

- `is_active_at` — true if valid_from ≤ at < valid_to (valid-time query).
- `was_known_at` — true if recorded_at ≤ at < superseded_at (transaction-time query).
- `supersede` — sets `superseded_at` on an existing node; does not delete.

### Cross-ADR alignment

- **ADR-042**: Claim gains bi-temporal fields; EvidenceBundle gains
  `recorded_at` to track when evidence was extracted.
- **ADR-043**: process-plane nodes (Hypothesis, ResultComparison) gain
  bi-temporal fields; ExperimentRun gets `valid_from` = run timestamp,
  `valid_to` = OPEN until re-run.
- **ADR-044**: schema_version bumps to 2 on fact-bearing schemas when
  bi-temporal fields become required (Phase 3).
- **ADR-045**: validator gains a `temporal-consistency` rule that flags
  fact-bearing nodes missing `recorded_at`.

## Alternatives considered

1. **Event-sourced log instead of mutable fields** — append-only log of
   changes. Pro: full history. Con: requires query-time replay to determine
   current state; complex for read-heavy paths. Rejected for daily-archive's
   scale and read patterns.
2. **Separate TemporalVersion node type** — one node per fact, one edge per
   version. Pro: clean separation. Con: graph explosion (5-10× nodes).
   Rejected unless audit pressure forces reconsideration.
3. **Single `valid_from` only** (current). Rejected — cannot answer
   transaction-time questions, which are the auditor questions.

## Consequences

- Every fact-bearing schema gains 3 new optional fields initially, 3
  required fields eventually. Schema versions bump.
- Validator gains a temporal-consistency rule (ADR-045 Wave F extension).
- Future `da graph-at --timestamp YYYY-MM-DD` CLI command becomes possible.
- Storage cost: 3 extra DateTime fields per fact-bearing node (~24 bytes
  each, ~72 bytes total). Negligible.
- Migration: one-time back-fill of `recorded_at` on existing fact-bearing
  nodes; `valid_to` and `superseded_at` default to OPEN.

## Open questions

- Should Author and Institution be fact-bearing? (Author affiliation changes
  over time.) Defer to ADR-048 Decision Intelligence — affiliation is a
  decision event.
- Should Source be bi-temporal? (Reliability tier can change.) Defer until
  first concrete use case.
