# Schema Validator — Design and Operation

> Companion to [ADR-045](adr/ADR-045-schema-validator-logic-relations-invariants.md).
> Status: implemented (Wave A–C, D-foundation, E, F, G). Runtime wiring
> (Wave D) is integrated into extraction + batch_ingest tests via
> assert_graph_conforms; production-store enforcement is future work.
> Companion commands: `da schema-check`, `da schema-list`, `da edge-contracts`,
> `da cross-refs`, `da validate-node`.

## Module location

```
crates/da-domain/src/validator.rs     ← pure-logic validator
crates/da-application/tests/schema_audit_test.rs  ← CI guard
```

The validator lives in `da-domain` (logic-only, no IO). It depends only
on `schema::all_node_schemas()` and `relation::{structure,bibliographic,hypergraph}`.

## Public API

```rust
pub enum Severity { Critical, Warning }

pub struct SchemaViolation {
    pub severity: Severity,
    pub rule:      String,   // e.g. "required-field", "D127-fail-closed"
    pub field:     Option<String>,
    pub message:   String,
}

pub type PropertySnapshot = HashMap<String, serde_json::Value>;

pub fn validate_node_properties(
    label: &str,
    props: &PropertySnapshot,
) -> Vec<SchemaViolation>;

pub fn validate_edge_type(edge_type: &str) -> Option<SchemaViolation>;

pub fn format_violations(violations: &[SchemaViolation]) -> String;
```

## Rules

| Rule                       | Sev     | Applies to        | What it catches                                          |
|----------------------------|---------|-------------------|----------------------------------------------------------|
| `schema-registry`          | Crit    | node              | Unknown label (not in `all_node_schemas`)                |
| `required-field`           | Crit    | node              | Required field missing or null                           |
| `D127-fail-closed`         | Crit    | node (all labels) | `import_eligible` missing or non-boolean                 |
| `D134-retrieval-elig`      | Crit    | node (all labels) | `retrieval_eligible` missing or non-boolean              |
| `ADR-044-schema-version`   | Crit    | node (all labels) | `schema_version` missing or not a positive integer       |
| `ADR-040-vid`              | Crit    | node (all labels) | `vid` missing, empty, or non-string                      |
| `type-mismatch`            | Warn    | node              | Value type ≠ declared `FieldType`                        |
| `unknown-field`            | Warn    | node              | Property key not declared in required OR optional        |
| `edge-registry`            | Crit    | edge              | Edge type not in `relation::{structure,bibliographic,hypergraph}` |

Severity rule of thumb: Critical = missing required field, broken
invariant, unknown label/edge. Warning = type drift or unknown optional
field. Critical violations must be fixed; warnings should be triaged.

## How to invoke

### In a test

```rust
use da_domain::validator::{validate_node_properties, format_violations, PropertySnapshot};
use serde_json::json;

let mut props = PropertySnapshot::new();
props.insert("vid".to_string(), json!("vid:paper:1234.5678"));
props.insert("arxiv_id".to_string(), json!("1234.5678"));
props.insert("title".to_string(), json!("My Paper"));
props.insert("valid_from".to_string(), json!(1234567890_i64));
props.insert("import_eligible".to_string(), json!(false));
props.insert("retrieval_eligible".to_string(), json!(true));
props.insert("schema_version".to_string(), json!(1_i64));

let v = validate_node_properties("Paper", &props);
assert!(v.is_empty(), "unexpected violations:\n{}", format_violations(&v));
```

### Smoke test for all 29 schemas

`validator::tests::test_all_node_schemas_produce_a_valid_minimum_node` builds
a synthetic minimum-valid node for every registered schema and asserts
zero Critical violations. This guards against future schemas that declare
required fields of types the validator does not synthesize correctly.

### CI guard for create_node sites

`schema_audit_test::test_all_pipeline_node_labels_have_registered_schemas`
scans `da-application/src/*.rs` for `create_node("Label")` call sites
and asserts every label is registered. A new node type in the pipeline
without a Schema struct + `all_node_schemas()` entry fails this test.

## Bugs found and fixed by the validator audit (2026-07-24)

| Node             | Bug                          | Schema rule violated         |
|------------------|------------------------------|------------------------------|
| `Category`       | Missing `is_primary`         | `required-field`             |
| `SchedulerTask`  | Missing `vid`                | `required-field`, `ADR-040`  |
| `MetricObservation` | Missing `run_id`          | `required-field`             |
| Mock (test infra) | `find_node_by_string_property` ignored `_label` | MEM495 |

All four were closed in the same wave as the validator introduction.

## Limitations

- **Not wired into runtime create_node paths.** Use cases still call
  `graph_store.create_node(label)` + `set_node_property_*` directly. The
  validator only runs at test time today. Wave D will capture snapshots
  of actual writes and run the validator on them.
- **Does not validate cross-node references.** A `vid:obs:...:accuracy`
  value may look well-formed but reference a non-existent MetricDefinition.
  Cross-reference validation needs a graph read path — out of scope.
- **Source-code auditor is regex-free but line-based.** It does not
  match create_node calls spanning multiple lines or hidden behind
  macros. Practical impact: low (the pipeline does not use such patterns).
- **Edge validator only checks the edge type string.** It does not check
  that source/target node labels are compatible with the edge semantics
  (e.g. `AUTHORED_BY` should go Paper→Author). Future work.

## Future waves

- **Wave D**: capture property snapshots from live `set_node_property_*`
  calls and run validator on each node after write. Surface Critical
  violations as structured logs.
- **Wave E (done)**: CLI command `da schema-check` that runs the audit hook
  across all crates and prints a markdown report.
- **Wave F**: cross-reference validator — given a `vid` field, confirm
  a node with that `vid` exists (requires read path).
- **Wave G**: edge endpoint validator — given an edge type, check that
  source and target labels match the edge's semantic contract.
