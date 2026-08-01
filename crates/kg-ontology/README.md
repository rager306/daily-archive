# kg-ontology

Universal knowledge graph ontology crate for Rust.

Reusable schema-driven validation, bi-temporal fact helpers, and (in
future phases) YAML-driven ontology loading, edge contracts, aspects,
versioning, and standard vocabulary mappings.

## Status

**Phase A** (current): skeleton crate with universal validator types
(`Severity`, `Violation`, `PropertySnapshot`, `format_violations`) and
bi-temporal helpers (`is_active_at`, `was_known_at`, `is_current`,
`validate_bitemporal`). 14 tests.

**Phase B-F** (planned per ADR-050): YAML ontology loader, migration
from `da-domain` schemas, aspects, versioning, cross-project reuse.

## Usage

```toml
[dependencies]
kg-ontology = { path = "crates/kg-ontology" }
```

```rust
use kg_ontology::{Severity, Violation, format_violations};
use kg_ontology::temporal::{is_active_at, OPEN};

// Phase A: use the universal types
let violation = Violation::critical("required-field", "vid", "missing");
assert_eq!(violation.severity, Severity::Critical);

// Phase D (future): validate against a YAML-loaded ontology
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
