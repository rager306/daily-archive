- [ADR-045: Schema Validator — Logic, Relations, and Invariant Checks](#adr-045-schema-validator--logic-relations-and-invariant-checks)
  - [Status](#status)
  - [Context](#context)
  - [Decision](#decision)
  - [Approaches Considered](#approaches-considered)
  - [Consequences](#consequences)
  - [Alternatives](#alternatives)
  - [Implementation Wave](#implementation-wave)

# ADR-045: Schema Validator — Logic, Relations, and Invariant Checks

## Status

Proposed (2026-07-24). Implementation: `crates/da-domain/src/validator.rs` + audit
test in `crates/da-application/tests/schema_audit_test.rs`.

## Context

The pipeline materializes nodes from many use cases (ingest, extraction,
enrich, cluster, scheduler, healing). Each `create_node("Label")` site
must set:

1. All `required_fields()` declared on the label's `NodeSchemaDef` impl.
2. The architectural invariants (D127 fail-closed `import_eligible=false`,
   D134 `retrieval_eligible`, ADR-044 `schema_version>=1`, ADR-040
   non-empty `vid`).
3. Only fields declared in the schema (required OR optional) — no ad-hoc
   keys.

The existing `NodeSchemaDef::validate()` only checked presence of required
fields, returned a single error, ignored types, and did not enforce
invariants. As a result, three schema violations slipped into the
materialized pipeline and survived multiple reviews:

- `Category` missing required `is_primary`.
- `SchedulerTask` missing required `vid`.
- `MetricObservation` missing required `run_id`.

A secondary gap surfaced during the audit: the batch-ingest mock
`find_node_by_string_property` ignored its `_label` parameter, so
dedup-by-key lookups matched across unrelated node types (Reference and
Citation both store `arxiv_id`). The real adapter filters by label; the
mock did not, hiding missing Citation creation.

## Decision

Add a dedicated `validator` module in `da-domain` that checks logic,
relations, and invariants. Pure logic, no IO. Returns ALL violations
from a single pass, with severity levels.

```text
validate_node_properties(label, &PropertySnapshot) -> Vec<SchemaViolation>
validate_edge_type(edge_type) -> Option<SchemaViolation>
```

### Rules checked

| Rule                     | Severity | What it catches                                   |
|--------------------------|----------|---------------------------------------------------|
| `schema-registry`        | Critical | `create_node("X")` where X is not registered      |
| `required-field`         | Critical | Required field absent or null                     |
| `D127-fail-closed`       | Critical | `import_eligible` missing or non-boolean          |
| `D134-retrieval-elig`    | Critical | `retrieval_eligible` missing or non-boolean       |
| `ADR-044-schema-version` | Critical | `schema_version` missing or not a positive int    |
| `ADR-040-vid`            | Critical | `vid` missing, empty, or non-string               |
| `type-mismatch`          | Warning  | Value type does not match declared `FieldType`    |
| `unknown-field`          | Warning  | Property key not declared in required OR optional |
| `edge-registry`          | Critical | Edge type not in `relation::structure/biblio/hg`  |

### Audit hook (compile-time guard)

A test (`schema_audit_test.rs`) scans `crates/da-application/src/*.rs`
for `create_node("Label")` call sites and asserts:

1. Every referenced label is in `all_node_schemas()`.
2. The pipeline materializes the required publication-plane node set.

This guards against future regressions where a new node type is
introduced in the pipeline without registering its schema.

## Approaches Considered

### A. Runtime validator integrated in create_node (REJECTED)

Wrap every `graph_store.create_node()` in a use-case method
`create_validated_node(label, props)` that validates before write.

- Pro: catches at write time.
- Con: requires property bag upfront — current pipeline writes properties
  one-by-one via `set_node_property_*`; refactor cost high.
- Con: runtime overhead on hot path.

### B. Post-write read-back audit (REJECTED)

After each use case, read nodes back from the store and validate.

- Pro: validates actual DB state.
- Con: needs a read-back path that does not exist uniformly.
- Con: does not catch the moment of write — errors surface late.

### C. Pure source-code auditor (PARTIALLY ADOPTED)

A test that parses `create_node` sites with regex and checks them.

- Pro: catches on CI; no runtime cost.
- Con: fragile to formatting; cannot check property values.
- Adoption: kept as audit guard (see Audit hook above) — catches the
  "label registered" invariant only. Logic/value validation delegated
  to Approach D.

### D. Hybrid: validator module + tests + audit macro (CHOSEN)

Combine Approach C's audit guard with a pure-logic validator module.
The validator returns ALL violations; tests drive the validator against
synthetic minimum-valid nodes for all 29 schemas; the audit macro guards
the "label registered" rule.

- Pro: catches missing fields, broken invariants, unknown labels in one
  pass; runs in CI; no runtime overhead on pipeline.
- Pro: validator lives in da-domain (logic-only, no IO) — preserves
  onion layering.
- Con: does not validate actual DB state — only the property snapshots
  that the pipeline claims to write. Mitigation: a future Slice can add
  a "snapshot writer" that captures the actual `set_node_property_*`
  calls per node and feeds them to the validator.

## Consequences

- Adding a new node type now requires registering its schema in
  `all_node_schemas()` — the audit test fails otherwise.
- The validator is the single source of truth for "what a valid node of
  label X looks like" — Publication, Process, and Edge planes all share
  one rule set.
- The validator is NOT yet wired into runtime create_node paths (future
  work — see Implementation Wave). It currently runs at test time only.
- Mock graph stores must honor the real adapter's contract (label-aware
  `find_node_by_string_property`). The audit revealed one mock that did
  not; the fix (MEM495) is the pattern for all future mocks.

## Alternatives

- **Procedural macros** that generate builders per schema: would catch
  missing fields at compile time, but require significant macro
  infrastructure and do not help with runtime property bags.
- **serde-based round-trip validation**: serialize node properties to
  JSON, deserialize into a typed struct per schema. Heavy refactor;
  does not cover invariants cleanly.

## Implementation Wave

- **Wave A (done)**: validator module + tests for all 29 schemas + edge
  registry check.
- **Wave B (done)**: audit-time test guarding `create_node` label
  registration.
- **Wave C (done)**: apply validator to current pipeline; fix 3 schema
  violations (Category/SchedulerTask/MetricObservation); fix mock label
  contract gap (MEM495); fix Reference dedup logic.
- **Wave D (future)**: optional runtime validator wrapper — when a
  use case finishes writing a node, capture its property snapshot and
  run the validator. Surface failures as structured logs.
- **Wave E (future)**: a CLI command (`da schema-check`) that imports
  the audit hook and runs it across all crates, for ad-hoc checks
  outside CI.
