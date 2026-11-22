# kg-ontology

Universal knowledge graph ontology crate for Rust.

Reusable schema-driven validation, bi-temporal fact helpers, and (in
future phases) YAML-driven ontology loading, edge contracts, aspects,
versioning, and standard vocabulary mappings.

## Status

**Current**: YAML-driven ontology registry and validation surface. Modules:
`registry` (OntologyRegistry loading node schemas, identity/property indexes,
edge contracts and stage contracts from `data/ontology/*.yaml`), `schema`,
`edge_contract`, `identity_index`, `vid`, `validator`, `temporal` +
`bitemporal` helpers (`is_active_at`, `was_known_at`, `is_current`,
`validate_bitemporal`), `healing` and `retry`. 119 tests.

**In progress** (per ADR-050): closing the bounded detect-only contract-audit
gaps (endpoint/topology/consumer coverage), a supported acceptance runner and
absent semantic layers (NodeSchemaDef → OntologyRegistry).

## Usage

```toml
[dependencies]
kg-ontology = { path = "crates/kg-ontology" }
```

```rust
use kg_ontology::{Severity, Violation, format_violations};
use kg_ontology::temporal::{is_active_at, OPEN};

// Use the universal types
let violation = Violation::critical("required-field", "vid", "missing");
assert_eq!(violation.severity, Severity::Critical);

// Validate against a YAML-loaded ontology
// let registry = OntologyRegistry::load()?;
// let violations = registry.validate_node("Paper", &snapshot);
```

## Design

- **Zero project-specific dependencies.** This crate does not know
  about `Paper`, `Claim`, or any domain. It works against generic
  `PropertySnapshot` inputs.
- **Data, not code.** When Phase D ships, all schema data lives in
  YAML (`data/ontology/*.yaml`). Rust holds loader + validator logic.
- See [ADR-050](../doc/adr/ADR-050-universal-graph-subsystem-kg-crate-family.md)
  for the full architecture decision.

## License

MIT
