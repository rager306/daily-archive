# ADR-063: GraphWrite stays deferred-by-design

- Status: accepted
- Date: 2026-08-20
- Deciders: agent (analysis), context from ADR-037/056
- Related: ADR-037 (scheduler priority table), ADR-056 (load-plane task
  mapping), D278 / D127

## Context

`GraphWrite` is listed as a `TaskType` with `Low` priority in the scheduler
(ADR-037) and mapped in ADR-056 as `deferred graph write batch (if any)` — a
conditional, "if any" row. This milestone (M417) evaluates whether a real
producer (and therefore an executor) exists today.

Findings from the codebase audit:

- There is **no deferred graph-write queue** anywhere. All graph writes
  (ingest materialization, extraction materialization, candidate
  materialization, vector writes) happen **inline** during the journey/stage
  execution, under `import_eligible=false` (D127).
- The only "pending" mechanism, `openalex_pending` Work stubs, already has its
  own scheduler type `OpenAlexEnrich` (`OpenAlexEnrichExecutor`). It is not a
  candidate for GraphWrite.
- No stage, pipeline, or CLI path produces a "batch of pending graph writes"
  that a scheduler worker would later flush.

Per the project's anti-fake rule ("an executor without honest inputs is a
fake; an executor without honest inputs is not a valid delivery"), building a
`GraphWriteExecutor` and a producer now would require inventing a deferred-write
seam that does not exist.

## Decision

Keep `GraphWrite` **deferred-by-design**:

1. Do **not** add any `GraphWriteExecutor` or `GraphWrite` producer in M417 or
   its successors until a genuine deferred graph-write source exists (e.g. a
   bulk/batched materialization pathway or an off-line write queue).
2. Add a Governor guard (`scheduler_no_graph_write_executor`) that asserts no
   registered `GraphWrite` executor path appears in `scheduler_runner`,
   preventing a fake revival.
3. Record this as decision D278 so the intent is durable.

## Consequences

- No fake scheduler seam is introduced; unregistered `GraphWrite` remains
  fail-closed (a pending `graph_write` task would go unclaimed / no_executor).
- When a real deferred-write producer appears, revisit ADR-063, lift the
  guard, and build the executor to the M410–M416 pattern.
- D127 import lock is unchanged: inline graph writes stay
  `import_eligible=false`.

## Compliance

- Governor marker `scheduler_no_graph_write_executor` requires that
  `scheduler_runner.rs` does not register a GraphWrite executor.
- M417 S02 closes this decision; future milestones that want to realize
  GraphWrite must first propose the honest producer in an ADR.
