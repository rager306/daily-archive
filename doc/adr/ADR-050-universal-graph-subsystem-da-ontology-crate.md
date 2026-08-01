# ADR-050: Universal Graph Subsystem — `da-ontology` Crate

**Status:** Proposed
**Date:** 2026-07-24
**Deciders:** collaborative
**Related:**
- ADR-040 (Samyama schemaless sole store)
- ADR-044 (schema lifecycle, versioning)
- ADR-045 (validator, currently hardcoded schemas)
- ADR-046 (bi-temporal facts)
- ADR-047 (conflicts)
- ADR-048 (decisions)
- ADR-049 (pipeline DSL)
- ONTOLOGY-DESIGN-V2.md, ONTOLOGY-COMPLETENESS-AUDIT.md
- Research: [OntoKG (arxiv 2604.02618v1)](https://arxiv.org/html/2604.02618v1),
  [Neotoma Schema Registry](https://github.com/markmhendrickson/neotoma/blob/main/docs/subsystems/schema_registry.md),
  [BioCypher](https://biocypher.org/BioCypher/reference/schema-config/),
  [yaml2graph](https://github.com/alishams21/yaml2graph),
  [Neo4j GRAPH TYPE](https://neo4j.com/blog/developer/graph-type-schema-enforcement-made-easy-preview/),
  [Practical Advice for Ontology Engineering](https://gdotv.com/blog/ontology-modelling-rdf-property-graphs/),
  [Schema Design Best Practices](https://kindatechnical.com/knowledge-graphs/schema-design-best-practices-for-knowledge-graphs.html),
  [KG-ER conceptual schema](https://arxiv.org/pdf/2508.02548),
  [RDF 1.2 vs Neo4j](https://ontologist.substack.com/p/rdf-12-vs-neo4jopencypher),
  [Semantica](https://github.com/semantica-agi/semantica)

## Context

The current daily-archive ontology is encoded as **Rust struct implementations
of the `NodeSchemaDef` trait**: 31 `XSchema` structs across 7 files in
`da-domain`, each returning `required_fields()` and `optional_fields()` from
hardcoded `vec![...]` literals. Adding a field or a node type requires
editing Rust source, recompiling, and shipping a new binary — even though
the underlying storage (Samyama) is schemaless and would accept any
property on any node.

This architecture contradicts the binding project directive **"не
хардкодим"** (all reference data in YAML; Rust holds logic only) and the
research-backed consensus that KG schemas should be:

- **Data, not code** — declarative YAML/JSON/SHACL artefacts.
- **Runtime-evolvable** — adding a field without redeploying.
- **Versioned & migrated** — old nodes stay readable as schemas evolve.
- **Portable** — same ontology applies across storage backends.
- **Reusable** — usable by agents, pipelines, validators, query builders,
  and downstream tools without code coupling.

Cross-pollination research (Semantica, OntoKG, BioCypher, Neotoma,
yaml2graph, Neo4j GRAPH TYPE) shows five orthogonal concerns collapsed
into the single `NodeSchemaDef` trait:

1. **Type identity** (this is a Paper).
2. **Property taxonomy** (which properties exist, their types, cardinality).
3. **Intrinsic/relational routing** (which properties are node attributes
   vs which become traversable edges).
4. **Inheritance/aspects** (Claim is_a Proposition; provenance applies to
   all fact-bearing types).
5. **Validation rules** (SHACL-like constraints, not just field presence).

The current trait models #1 and a thin slice of #2 only.

### Reuse motivation

The graph subsystem — schema, validator, ontology loader, pipeline
routing rules — is **not specific to daily-archive**. It applies equally
to:

- `reactivegraph` (separate project) which builds knowledge graphs from
  source code and documentation.
- Future Rust agents that want a typed view of a schemaless graph.
- Any project that ingests heterogeneous sources into a property graph
  and needs schema-driven validation, extraction, and provenance.

If the graph concerns stay mixed into `da-domain` (which also holds
daily-archive-specific types like `Paper`, `Citation`, `ResearchProblem`),
they cannot be reused without dragging the scientific-paper domain along.
The fix is a **separate crate** that holds graph concerns only.

## Decision

Introduce a new crate **`da-ontology`** that holds the universal graph
subsystem. Move all schema-as-data, ontology, validator, and
graph-routing logic out of `da-domain` into it. `da-domain` keeps only
daily-archive-specific domain types (Paper, Claim, ResearchProblem) as
**thin typed views** over the generic substrate.

### Hexagonal / onion placement

```
                    ┌──────────────────────────────────────┐
                    │   da-cli                             │
                    │   (composition root + CLI commands)  │
                    └────────────┬─────────────────────────┘
                                 │
                    ┌────────────┴─────────────────────────┐
                    │   da-application                      │
                    │   IngestUseCase, ExtractionUseCase,   │
                    │   EnrichUseCase, ConflictDetector,    │
                    │   DecisionRecorder, Pipeline          │
                    └────────────┬─────────────────────────┘
                                 │
                ┌────────────────┼────────────────┐
                │                │                │
       ┌────────┴───────┐ ┌──────┴───────┐ ┌──────┴────────┐
       │  da-ports      │ │ da-adapters  │ │ da-graph      │
       │  GraphStore,   │ │ Samyama,     │ │ Cypher query  │
       │  Embedder,     │ │ OpenAlex,    │ │ builders      │
       │  Extractor     │ │ GROBID       │ │ + schema DDL  │
       └────────┬───────┘ └──────────────┘ └───────────────┘
                │
       ┌────────┴──────────────────────────────────┐
       │  da-domain                                │  ← project-specific
       │  Paper, Citation, Claim (typed views),    │
       │  ResearchProblem, EvidenceBundle          │
       └────────────┬──────────────────────────────┘
                    │ depends on (downward)
       ┌────────────┴──────────────────────────────┐
       │  da-ontology  (NEW — universal)           │  ← reusable
       │  OntologyRegistry (YAML loader),          │
       │  LoadedSchema, PropertyDef, EdgeContract, │
       │  Aspect, ValidationRule, SchemaVersion,   │
       │  Validator, IntrinsicRelational router,   │
       │  BiTemporal helpers, CausalChain walker,  │
       │  StandardMappings (schema.org, FaBiO, …)  │
       └───────────────────────────────────────────┘
```

**Dependency rule (onion)**: dependencies point inward / downward.
`da-ontology` is the **innermost** layer of the graph stack — it has
zero project-specific dependencies. `da-domain` depends on
`da-ontology` (for `LoadedSchema`, `Validator`, etc.) but
`da-ontology` never depends on `da-domain`. Everything above
(`da-ports`, `da-adapters`, `da-application`, `da-cli`) transitively
uses `da-ontology` through `da-domain`.

**Reuse boundary**: `da-ontology` ships as a standalone crate. Any Rust
project that needs schema-driven property-graph validation can add
`da-ontology = "..."` to its `Cargo.toml`, supply its own
`ontology/` YAML files, and get:

- YAML ontology loader (node types, edge types, aspects, rules).
- Generic validator (runs against any `PropertySnapshot`).
- Intrinsic/relational routing decisions.
- Bi-temporal fact helpers.
- Causal-chain walker for Decision graphs.
- Schema versioning + migration policy lookup.

The crate has **no** daily-archive imports — no `Paper`, no
`ResearchProblem`, no Samyama client, no GROBID. It is pure logic over
data loaded from YAML.

### Cargo workspace layout

```toml
# Cargo.toml (workspace)
[workspace]
members = [
    "crates/da-ontology",   # NEW: universal graph subsystem
    "crates/da-domain",
    "crates/da-ports",
    "crates/da-application",
    "crates/da-adapters",
    "crates/da-graph",
    "crates/da-cli",
]

[workspace.dependencies]
da-ontology = { path = "crates/da-ontology" }   # NEW
da-domain = { path = "crates/da-domain" }
# …
```

`da-ontology` has only third-party deps (`serde`, `serde_yaml`,
`chrono`, `once_cell`/`std::sync::LazyLock`, `regex`). No
`da-*` deps.

### What moves where

| Currently in                | Concern                                | Moves to         |
|-----------------------------|----------------------------------------|------------------|
| `da-domain::schema`         | `NodeSchemaDef` trait, `FieldType`, `all_node_schemas()` | `da-ontology::schema` (loaded from YAML) |
| `da-domain::edge_contract`  | `EdgeContract`, `edge_contracts()` registry | `da-ontology::edge_contract` (loaded from YAML) |
| `da-domain::validator`      | `validate_node_properties`, `CrossReferenceField`, severity, violations | `da-ontology::validator` |
| `da-domain::temporal`       | `BiTemporalFact` helpers, `OPEN` sentinel | `da-ontology::temporal` |
| `da-domain::relation`       | Edge string constants + modules        | **stays** — `da-domain::relation` keeps the constant strings (they are cheap and ubiquitous); `da-ontology::edge_contract` references them by string only |
| `da-domain::paper` (struct) | `Paper` typed view (logic-specific)    | **stays** in `da-domain` (project-specific) |
| `da-domain::paper` (Schema) | `PaperSchema` struct                   | moves to `data/ontology/node_types.yaml`; `Paper` Rust struct keeps typed accessor methods that read from a generic `Node` |
| `da-graph::schema`          | Samyama-specific DDL strings           | **stays** — this is adapter-specific, not universal |
| `da-graph::queries`         | Cypher builders                        | **stays** — but reads label/field names from `da-ontology` at runtime |
| `da-application::pipeline`  | Pipeline DSL (ADR-049)                 | **stays** — pipeline orchestration is project-specific; the DSL can move to `da-ontology` if another project wants it, defer |

### What stays in da-domain

- **Typed view structs** (`Paper`, `Author`, `Claim`, `ResearchProblem`).
  These are thin Rust newtypes around a generic `Node` with typed accessor
  methods (`paper.arxiv_id() -> Option<&str>`). They exist for hot-path
  type safety and IDE autocomplete, but **do not carry schema metadata**.
- **Edge constant strings** (`AUTHORED_BY`, `MENTIONS`, etc.) — cheap,
  stable, used pervasively in pipeline code. Moving them to
  `da-ontology` would force every pipeline file through an extra import.
- **Daily-archive-specific domain logic** (`vid::paper_vid`,
  `entity::EntityType` enum, `healing::HealingOperation`).

### What lives in da-ontology

Seven sub-modules matching the seven ontology layers from the research
synthesis:

```text
crates/da-ontology/
├── Cargo.toml
├── src/
│   ├── lib.rs                  # public re-exports
│   ├── schema.rs               # LoadedSchema, NodeType, PropertyDef
│   ├── edge_contract.rs        # EdgeContract, EdgeEndpoint
│   ├── aspects.rs              # Aspect (mixin), AspectApplication
│   ├── validation.rs           # ValidationRule, Severity, Validator
│   ├── versioning.rs           # SchemaVersion, MigrationPolicy
│   ├── routing.rs              # Intrinsic/Relational routing decisions
│   ├── temporal.rs             # BiTemporalFact helpers (moved from da-domain)
│   ├── causal.rs               # CausalChain walker for Decisions (ADR-048)
│   ├── mappings.rs             # Standard vocab mappings (schema.org, FaBiO)
│   ├── loader.rs               # YAML loader + cached registry
│   └── registry.rs             # OntologyRegistry: top-level API
├── tests/
│   ├── yaml_loader_tests.rs
│   ├── validator_tests.rs
│   └── fixtures/
│       └── ontology/           # example ontology for tests
└── README.md                   # how to reuse in another project
```

### Public API of da-ontology

```rust
// crates/da-ontology/src/lib.rs

pub use crate::schema::{LoadedSchema, NodeType, PropertyDef, FieldType, Cardinality};
pub use crate::edge_contract::{EdgeContract, EdgeEndpoint};
pub use crate::aspects::{Aspect, AspectApplication};
pub use crate::validation::{ValidationRule, Severity, Violation, Validator};
pub use crate::versioning::{SchemaVersion, MigrationPolicy, VersionedSchema};
pub use crate::routing::{PropertyRouting, RoutingDecision};
pub use crate::temporal::{self, OPEN, is_active_at, was_known_at};
pub use crate::causal::{CausalChain, CausalRelation};
pub use crate::mappings::{StandardVocab, VocabularyMapping};
pub use crate::registry::{OntologyRegistry, RegistryError};

/// Top-level entry point. Loads YAML ontology from disk
/// (or bundled fallback) and exposes a cached registry.
pub struct OntologyRegistry {
    node_types: HashMap<String, NodeType>,
    edge_types: HashMap<String, EdgeContract>,
    aspects: HashMap<String, Aspect>,
    rules: Vec<ValidationRule>,
    versions: HashMap<String, Vec<SchemaVersion>>,
}

impl OntologyRegistry {
    pub fn load() -> Result<Self, RegistryError>;
    pub fn load_from_dir(path: &Path) -> Result<Self, RegistryError>;
    pub fn validate_node(&self, label: &str, props: &PropertySnapshot) -> Vec<Violation>;
    pub fn validate_edge(&self, edge_type: &str, source_label: &str, target_label: &str) -> Option<Violation>;
    pub fn route_property(&self, label: &str, property: &str) -> RoutingDecision;
    pub fn active_schema_version(&self, label: &str) -> Option<&SchemaVersion>;
    pub fn migrate_snapshot(&self, label: &str, from: &str, to: &str, props: PropertySnapshot) -> Result<PropertySnapshot, RegistryError>;
    pub fn aspects_for(&self, label: &str) -> Vec<&Aspect>;
    pub fn standard_mapping(&self, label: &str, vocab: StandardVocab) -> Option<&str>;
}
```

### YAML ontology layout

All ontology data lives in `data/ontology/` (daily-archive) or a
caller-supplied directory (other projects). Seven files matching the
seven concerns identified in the research synthesis:

```text
data/ontology/
├── node_types.yaml          # Layer A — entity taxonomy
├── properties.yaml          # Layer B — property definitions (intrinsic/relational)
├── edge_types.yaml          # Layer C — relationship contracts
├── aspects.yaml             # Layer D — cross-cutting traits (provenance, bi-temporal)
├── validation_rules.yaml    # Layer E — SHACL-like constraints
├── schema_versions.yaml     # Layer F — semantic versioning + migrations
└── dictionaries.yaml        # Layer G — controlled vocabularies
```

`da-ontology` ships a **bundled fallback** (`include_str!` of a reference
ontology) so the crate works zero-config for evaluation; callers override
by placing files in `data/ontology/` (discovered via the standard search
path) or by calling `OntologyRegistry::load_from_dir()`.

### What the YAML looks like (illustrative — full spec in ONTOLOGY-V3-DESIGN.md)

`data/ontology/node_types.yaml`:
```yaml
node_types:
  Paper:
    parent: Work
    layer: L1
    description: "Scientific paper from arXiv/OpenAlex"
    abstract: false
    identity:
      canonical_name_fields: [arxiv_id]
    lifecycle_stages: [ingested, parsed, extracted, enriched, curated]
    aspects: [provenance, bi_temporal]
    standard_mappings:
      schema_org: "ScholarlyArticle"
      fabio: "Expression"
    storage:
      hot_path_typed: true   # da-domain exposes a typed Paper view
```

`data/ontology/properties.yaml`:
```yaml
properties:
  Paper.arxiv_id:
    type: string
    routing: intrinsic
    cardinality: 1..1
    indexed: unique
    validation: { regex: '^\d{4}\.\d{4,5}$' }
    lifecycle_stage: ingested
  Paper.references:
    routing: relational
    target_type: Reference
    cardinality: 0..N
    edge_label: HAS_PART
    lifecycle_stage: ingested
```

`data/ontology/aspects.yaml`:
```yaml
aspects:
  bi_temporal:
    applies_to: [Claim, EvidenceBundle, MetricObservation, Conflict, Decision]
    properties:
      valid_from: { type: datetime, required: true }
      valid_to:   { type: datetime, default: OPEN }
      recorded_at: { type: datetime, required: true }
      superseded_at: { type: datetime, default: OPEN }
  provenance:
    applies_to: [Claim, Entity, Reference, Decision]
    properties:
      source_vid: { type: string, required: true }
      extraction_method: { type: string, enum: [rule_based, llm, grobid] }
      confidence: { type: float, range: [0.0, 1.0] }
```

`data/ontology/schema_versions.yaml`:
```yaml
schema_versions:
  Paper:
    - version: "1.0.0"
      date: 2026-07-25
      changes: "initial"
      active: false
    - version: "1.1.0"
      date: 2026-07-29
      changes: "+scientific_domains"
      backward_compatible: true
      active: true
```

### Compatibility with existing ADRs

- **ADR-040 (Samyama sole store)**: unchanged. Samyama stays schemaless;
  `da-ontology` is an application-layer concern that validates and routes
  but does not enforce at write time inside the engine.
- **ADR-044 (schema lifecycle)**: the `schema_version` field on each
  node remains; it now references the active version of that node type's
  YAML entry, not a Rust constant.
- **ADR-045 (validator)**: the `NodeSchemaDef` trait, `all_node_schemas()`,
  and the hardcoded `XSchema` structs are **deprecated and removed**.
  The validator logic moves to `da-ontology::validator`, parameterised
  by the loaded ontology. The public function
  `validate_node_properties(label, props)` becomes
  `registry.validate_node(label, props)`.
- **ADR-046/047/048/049**: the bi-temporal helpers, causal-chain walker,
  conflict kinds, decision categories all move to `da-ontology`. Their
  **data** (which categories exist, which kinds are valid) moves to YAML;
  the **logic** (how to walk a causal chain, how to compute
  `is_active_at`) stays in Rust inside `da-ontology`.

### Migration path

1. **Phase A — new crate, parallel runs**:
   - Create `crates/da-ontology` with the public API above.
   - Move the temporal module and the generic validator types from
     `da-domain` into it.
   - `da-domain` re-exports `da-ontology` types for backward compatibility.
   - No YAML loader yet — registry still built from the existing
     `all_node_schemas()` function.
   - Tests pass; nothing user-visible changes.

2. **Phase B — YAML ontology files**:
   - Export every existing `XSchema` to `data/ontology/*.yaml` via a
     one-shot `da ontology export` command.
   - Implement `OntologyRegistry::load_from_dir()` and the cached
     `load()` entry point.
   - Validator now reads from the loaded registry.
   - `all_node_schemas()` becomes a **deprecated adapter** that calls
     into the registry and returns the same shape.

3. **Phase C — deprecate `XSchema` Rust structs**:
   - Mark `PaperSchema`, `ClaimSchema`, … as `#[deprecated]`.
   - Pipeline code switches to `registry.validate_node(label, props)`.
   - The typed view structs in `da-domain` (`Paper`, `Claim`) keep their
     accessor methods but no longer implement `NodeSchemaDef`.

4. **Phase D — removal**:
   - Delete the `XSchema` structs and `NodeSchemaDef` trait from
     `da-domain`.
   - `da-domain` depends on `da-ontology` for all schema concerns.
   - `data/ontology/*.yaml` is the single source of truth.

5. **Phase E — cross-project reuse**:
   - Publish `da-ontology` (rename to `kg-ontology` or similar if we want
     to drop the daily-archive prefix) as a standalone crate.
   - `reactivegraph` and future projects add it as a dependency, supply
     their own `ontology/` directory, and get the validator, routing,
     versioning, and temporal helpers for free.

## Alternatives considered

### 1. Keep everything in `da-domain`, just move to YAML loading

Rejected. `da-domain` would then contain both daily-archive-specific
types (`Paper`, `ResearchProblem`) and generic graph concerns
(SchemaRegistry, Validator). Any other project wanting the generic layer
would have to depend on `da-domain` and pull in daily-archive-specific
types, violating the reuse goal.

### 2. Use an existing Rust KG crate (e.g. `sophia`, `rdf-rs`, `rio`)

Rejected for now. These are RDF-oriented; they assume triple-store
semantics and IRI identity. daily-archive and `reactivegraph` use
labelled property graphs (Samyama/Neo4j). RDF crates would impose an
alien data model. We may revisit if we need RDF interop (Phase E+).

### 3. Adopt SHACL as the validation layer (via `srkl` or a future Rust SHACL engine)

Tempting for standards compliance, but the Rust SHACL ecosystem is
immature (no production-grade SHACL engine in Rust as of 2026-07). Our
`da-ontology::validator` will model SHACL-like rules in YAML, not adopt
the SHACL RDF syntax directly. If a mature Rust SHACL engine lands, the
YAML rules can be auto-translated to SHACL for interop.

### 4. Move everything to `da-graph`

Rejected. `da-graph` currently holds Samyama-specific DDL and Cypher
builders — it is an adapter concern, not a universal ontology concern.
Putting the universal ontology there would entangle it with the
specific storage backend.

### 5. Inline graph concerns into a future `reactivegraph` and duplicate

Rejected — duplication defeats the reuse goal. The whole point of the
new crate is to write the ontology layer once.

## Consequences

### Positive

- **Universal crate**: `da-ontology` is reusable in any Rust project that
  needs schema-driven property-graph validation. Add to `Cargo.toml`,
  supply `ontology/` directory, done.
- **"Не хардкодим" honoured**: every schema, edge contract, aspect,
  validation rule, and version migration lives in YAML. Rust code is
  loader logic + validator logic only.
- **Runtime evolution**: add a field by editing `node_types.yaml` and
  restarting the process. No Rust rebuild for schema changes.
- **Multi-backend**: the same ontology applies whether the storage is
  Samyama, Neo4j, or an in-memory mock. Storage-adapter code reads
  `route_property()` to decide what becomes an edge vs an attribute.
- **Clean onion layering**: `da-ontology` is the innermost layer with
  zero project-specific dependencies. `da-domain` sits on top of it and
  adds daily-archive-specific typed views.
- **Better cross-project consistency**: `reactivegraph` and
  `daily-archive` can share vocabularies, validation rules, and
  temporal helpers.

### Negative

- **One-time migration cost**: 31 `XSchema` structs must be exported to
  YAML and the trait removed. Estimated 3-4 sessions.
- **Loss of compile-time field-name checking**: today `PaperSchema`
  declares `"arxiv_id"` and a typo in pipeline code surfaces at compile
  time (the validator's `unknown-field` rule fails in tests). After the
  move, the field name is a string loaded from YAML — typos surface at
  runtime when the validator runs. Mitigation: pre-commit
  `da ontology validate` and integration tests.
- **Indirection**: pipeline code that used to call `PaperSchema` now
  calls `registry.node_type("Paper")`. Slightly more verbose; mitigated
  by typed view structs in `da-domain` for hot paths.
- **YAML schema is a new attack surface**: a malformed `node_types.yaml`
  could break loading. Mitigation: `da ontology validate` CLI command,
  strict serde deserialization, schema-of-schemas (a meta YAML that
  validates the ontology YAML).

### Risk: drift between YAML and Rust typed views

If `da-domain::Paper::arxiv_id()` references a field that the YAML does
not declare, runtime error. Mitigation: a `da ontology check-rust`
command scans `da-domain` source for typed accessor methods and confirms
each referenced field exists in the loaded ontology. Run in CI.

## Open questions

1. **Crate name**: `da-ontology` (daily-archive-specific prefix) vs
   `kg-ontology` (project-neutral, eases publication). Defer to Phase E
   when we publish; for now `da-ontology` matches the workspace.
2. **Hot-path typed views**: do we keep `Paper`, `Author`, `Claim` as
   structs in `da-domain` for hot-path type safety, or do we go
   fully-dynamic (`Node` + `get_str("arxiv_id")`) everywhere? This ADR
   assumes **hybrid** — typed views exist but are thin and optional.
   Revisit if the hybrid becomes a maintenance burden.
3. **Schema-of-schemas**: do we write a meta YAML that validates the
   ontology YAML itself, or do we rely on strict serde + tests?
   Tentative: strict serde + a `validate_ontology()` test for now; meta
   YAML if we hit maintenance pain.
4. **Pipeline integration**: ADR-049 Pipeline DSL reads stage
   definitions from YAML. Should the pipeline templates live in
   `da-ontology` (universal) or `data/pipeline/` (daily-archive-specific)?
   Tentative: templates stay project-specific; the DSL engine can move
   to `da-ontology` later if another project wants it.
5. **Versioning cross-refs**: when `Claim` schema bumps from 1.0 to 2.0,
   do all nodes pointing to old Claims need migration too? This is the
   SHACL `sh:nodeKind` problem. Defer to Phase D when migrations land.

## Phase 1 deliverables (next 1-2 sessions)

- `crates/da-ontology/` created with empty module shells + Cargo.toml.
- `data/ontology/` directory with the 7 YAML files (initially with just
  Paper, Claim, Conflict as worked examples).
- `OntologyRegistry::load_from_dir()` implemented and tested on the
  worked examples.
- This ADR marked **Accepted** in ADR-INDEX.md.

Full migration (Phases B-E) is tracked as follow-up waves; this ADR only
fixes the architecture decision.
