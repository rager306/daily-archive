# ADR-050: Universal Graph Subsystem — kg-* Crate Family

**Status:** Accepted
**Date:** 2026-07-24
**Deciders:** collaborative
**Related:**
- ADR-040 (Samyama schemaless sole store)
- ADR-044 (schema lifecycle, versioning)
- ADR-045 (validator — subsumed by kg-ontology)
- ADR-046 (bi-temporal facts — helpers move to kg-algorithms)
- ADR-047 (conflicts — logic moves to kg-algorithms, kinds to YAML)
- ADR-048 (decisions — logic moves to kg-algorithms, categories to YAML)
- ADR-049 (pipeline DSL — moves to kg-pipeline)
- ONTOLOGY-DESIGN-V2.md, ONTOLOGY-COMPLETENESS-AUDIT.md
- Research: OntoKG (arxiv 2604.02618v1), Neotoma Schema Registry,
  BioCypher, yaml2graph, Neo4j GRAPH TYPE, KG-ER, Semantica

## Context

daily-archive's graph subsystem — schema, validator, bi-temporal helpers,
GraphStore trait, Samyama adapter, embedder, RuVector integration,
conflict/decision logic, pipeline DSL — is **not specific to scientific
papers**. It applies to `reactivegraph`, `law-nexus`, and any future Rust
project that builds a schema-driven property-graph knowledge base on top
of Samyama + RuVector.

Today these universal concerns are mixed into project-specific crates:

```text
crates/
├── da-domain/         ← MIXED: universal schema/validator + Paper/Claim
├── da-ports/          ← MIXED: universal GraphStore + OpenAlex/Parser
├── da-application/    ← MIXED: universal healing + IngestPDF
├── da-adapters/       ← MIXED: universal Samyama + GROBID/OpenAlex
├── da-graph/          ← MIXED: universal Cypher builder + project DDL
└── da-cli/            ← project-specific
```

A second project wanting the graph substrate must either (a) depend on
`da-domain` and pull in `Paper`/`Citation`/`ResearchProblem` it does not
need, or (b) fork and strip — which defeats reuse.

The binding project directive **"не хардкодим"** (data in YAML, logic in
Rust) is also violated: 31 `XSchema` structs in `da-domain` encode
reference data as Rust `vec![...]` literals. Best practices 2025-2026
(OntoKG, Neotoma, BioCypher, yaml2graph, Neo4j GRAPH TYPE) are
unambiguous: KG schemas belong in declarative YAML, not in code.

## Decision

Split the universal graph subsystem into **five granular crates** under a
neutral `kg-*` prefix (kg = knowledge graph). Each crate has a single
responsibility, zero `da-*` dependencies, and is independently releasable.
Consumers (daily-archive, reactivegraph, future projects) pick which
crates they need.

### Crate family

```text
crates/
├── kg-ontology/       # YAML schemas + validator + versioning + aspects
├── kg-storage/        # GraphStore trait + Samyama adapter + Mock
├── kg-embeddings/     # Embedder trait + FdEmbedder + RuVector bridge
├── kg-algorithms/     # bi-temporal, causal chains, conflict/decision logic
├── kg-pipeline/       # Pipeline DSL + ExecutionEngine + templates
├── da-domain/         # daily-archive typed views (Paper, Claim, …)
├── da-application/    # daily-archive use cases (Ingest, Extract, Enrich)
├── da-adapters/       # daily-archive adapters (GROBID, OpenAlex, HtmlParser)
├── da-graph/          # daily-archive Cypher queries
└── da-cli/            # daily-archive CLI
```

### Hexagonal / onion layering

```
┌────────────────────────────────────────────────────────────────┐
│  da-cli                                                        │
│  (composition root + CLI commands)                             │
└─────────────────┬──────────────────────────────────────────────┘
                  │
┌─────────────────┴──────────────────────────────────────────────┐
│  da-application                                                │
│  IngestUseCase, ExtractionUseCase, EnrichUseCase,              │
│  ConflictDetectionUseCase, DecisionRecorder                    │
└─────────────────┬──────────────────────────────────────────────┘
                  │
┌─────────────────┴──────┬─────────────────────┬─────────────────┐
│  da-ports              │  da-adapters         │  da-graph       │
│  (project-specific     │  (project-specific  │  (project       │
│   port refinements)    │   adapters)         │   Cypher)       │
└─────────────────┬──────┴─────────────────────┴─────────────────┘
                  │
┌─────────────────┴──────────────────────────────────────────────┐
│  da-domain                                                     │
│  Paper, Claim, ResearchProblem (typed views over Node)         │
└─────────────────┬──────────────────────────────────────────────┘
                  │ depends on (downward)
┌─────────────────┴──────────────────────────────────────────────┐
│  kg-* crates (universal, reusable)                             │
│                                                                │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐    │
│  │ kg-ontology    │  │ kg-storage     │  │ kg-embeddings  │    │
│  │ schemas,       │  │ GraphStore     │  │ Embedder,      │    │
│  │ validator,     │  │ trait +        │  │ FdEmbedder,    │    │
│  │ versioning,    │  │ Samyama + Mock │  │ RuVector bridge│    │
│  │ aspects        │  │                │  │                │    │
│  └───────┬────────┘  └───────┬────────┘  └───────┬────────┘    │
│          │                   │                   │             │
│          └────────┬──────────┴───────────────────┘             │
│                   │                                           │
│          ┌────────┴────────┐  ┌────────────────┐               │
│          │ kg-algorithms   │  │ kg-pipeline    │               │
│          │ bi-temporal,    │  │ Pipeline DSL + │               │
│          │ causal chains,  │  │ ExecutionEngine│               │
│          │ conflict/decision logic            │               │
│          └─────────────────┘  └────────────────┘               │
└────────────────────────────────────────────────────────────────┘
```

**Dependency rule**: `kg-*` crates never depend on `da-*`. `da-*`
crates may depend on any `kg-*`. Within `kg-*`, dependencies are:
- `kg-ontology` — no kg-* deps
- `kg-storage` — depends on `kg-ontology` (for schema-driven DDL generation, optional)
- `kg-embeddings` — depends on `kg-ontology` (for embedding placement rules)
- `kg-algorithms` — depends on `kg-ontology` (for aspect/rule lookups) + `kg-storage` (for graph reads)
- `kg-pipeline` — depends on `kg-ontology` (for validator pre-flight)

### Per-crate scope

#### kg-ontology

**Single responsibility**: load and serve the declarative ontology.

Contents:
- `LoadedSchema`, `NodeType`, `PropertyDef`, `FieldType`, `Cardinality`
- `EdgeContract`, `EdgeEndpoint`
- `Aspect`, `AspectApplication` (mixin pattern for cross-cutting traits)
- `ValidationRule`, `Severity`, `Violation`, `Validator`
- `SchemaVersion`, `MigrationPolicy`, `VersionedSchema`
- `RoutingDecision` (intrinsic vs relational)
- `StandardVocab`, `VocabularyMapping` (schema.org, FaBiO, CiTO, PROV-O)
- `OntologyRegistry` — top-level API, YAML loader with `include_str!` fallback

Data (in caller's `data/ontology/`):
- `node_types.yaml` — Layer A: entity taxonomy + identity + lifecycle
- `properties.yaml` — Layer B: intrinsic/relational routing + cardinality
- `edge_types.yaml` — Layer C: relationship contracts
- `aspects.yaml` — Layer D: cross-cutting traits (provenance, bi-temporal)
- `validation_rules.yaml` — Layer E: SHACL-like constraints
- `schema_versions.yaml` — Layer F: semantic versioning + migrations
- `dictionaries.yaml` — Layer G: controlled vocabularies

Dependencies: `serde`, `serde_yaml`, `once_cell`/`LazyLock`, `regex`.
Zero `da-*`, zero other `kg-*`.

#### kg-storage

**Single responsibility**: graph storage abstraction + Samyama
implementation + in-memory mock for tests.

Contents:
- `GraphStore` trait (query/export/import — moved from da-ports)
- `DirectGraphStore` trait (CRUD — moved from da-ports)
- `GraphStoreError`, `QueryResult`, `VectorMetric`, `VectorSearchResult`
- `SamyamaGraphStore` adapter (moved from da-adapters)
- `MockGraphStore` (moved from da-application/tests/common/ — the
  consolidated shared mock with assert_graph_conforms)
- Optional: schema-driven DDL generator (reads NodeType from
  kg-ontology, emits CREATE INDEX statements)

Dependencies: `kg-ontology` (optional, for DDL), `samyama-sdk` (vendor),
`async-trait`, `tokio`, `serde_json`.

#### kg-embeddings

**Single responsibility**: vector embeddings + RuVector integration.

Contents:
- `Embedder` trait (moved from da-ports)
- `FdApiEmbedder` adapter (FastData API — moved from da-adapters)
- `RuVectorBridge` — integration with `/root/vendor-source/ruvector`
  for PPR, message passing, GNN forward pushes
- Vector index lifecycle helpers

Dependencies: `kg-ontology` (for embedding field routing),
`reqwest`/`http`, `tokio`, `serde_json`. Optional feature flag
`ruvector` for the RuVector bridge (so consumers who don't need GNN
don't pull the vendor crate).

#### kg-algorithms

**Single responsibility**: graph algorithms that operate on top of
kg-storage and use kg-ontology for rules.

Contents:
- `temporal` — BiTemporal helpers (moved from da-domain): OPEN sentinel,
  `is_active_at`, `was_known_at`, `is_current`, `validate_bitemporal`
- `causal` — CausalChain walker for Decision graphs (ADR-048): BFS/DFS
  over CAUSED/INFLUENCED edges, cycle detection, depth limits
- `conflict` — ConflictDetector logic (ADR-047): scan for factual/
  typological/temporal/metric disagreements, resolution strategy
  dispatch
- `decision` — DecisionRecorder logic (ADR-048): record_decision,
  find_precedents (semantic search hook), trace_chain, analyze_impact
- `healing` — generic healing operations (silence/correct/merge) that
  emit Decision records; daily-archive-specific healing rules stay in
  da-application
- `traversal` — BFS/DFS/PPR wrappers over kg-storage

Dependencies: `kg-ontology`, `kg-storage`, `kg-embeddings` (optional,
for precedent search via similarity), `petgraph` (for in-memory graph
algorithms).

#### kg-pipeline

**Single responsibility**: declarative pipeline orchestration.

Contents:
- `Pipeline`, `PipelineStep`, `PipelineStage` enum
- `PipelineBuilder` DSL
- `ExecutionEngine`, `ExecutionResult`, `PipelineStatus`
- `FailureHandler`, `RetryPolicy`, `FailureAction`
- `PipelineValidator` (pre-flight checks, uses kg-ontology validator)
- `PipelineTemplateManager` — loads YAML templates from
  `data/pipeline_templates/`

Dependencies: `kg-ontology`, `tokio`, `serde_yaml`.

**Stage implementations** (Ingest, Extract, Enrich) stay in
`da-application` — they are project-specific. `kg-pipeline` provides
the engine; consumers plug in their own `PipelineStage::Custom(Box<dyn
Stage>)` handlers.

### What stays in daily-archive (da-* crates)

- **da-domain**: typed view structs (`Paper`, `Author`, `Claim`,
  `ResearchProblem`, `EvidenceBundle`) with accessor methods that read
  from a generic `Node`. No `NodeSchemaDef` trait — schemas come from
  kg-ontology YAML.
- **da-ports**: project-specific port refinements (Parser, Extractor,
  OpenAlexClient) — universal GraphStore moves to kg-storage.
- **da-application**: `IngestUseCase`, `ExtractionUseCase`,
  `EnrichUseCase`, `ClusterUseCase`, `SchedulerUseCase`. Use cases call
  into kg-algorithms for healing/conflict/decision and kg-pipeline for
  orchestration.
- **da-adapters**: `GrobidParser`, `OpenAlexHttpAdapter`, `HtmlParser`,
  `RuleExtractor` — project-specific parsers and external service
  adapters.
- **da-graph**: daily-archive-specific Cypher query builders
  (`queries.rs`). Samyama-specific DDL (`schema.rs`) moves to
  kg-storage; project queries stay here.
- **da-cli**: CLI commands.

### Cargo workspace after migration

```toml
[workspace]
members = [
    # Universal kg-* crates (reusable)
    "crates/kg-ontology",
    "crates/kg-storage",
    "crates/kg-embeddings",
    "crates/kg-algorithms",
    "crates/kg-pipeline",
    # daily-archive-specific
    "crates/da-domain",
    "crates/da-ports",
    "crates/da-application",
    "crates/da-adapters",
    "crates/da-graph",
    "crates/da-cli",
]

[workspace.dependencies]
# Universal
kg-ontology = { path = "crates/kg-ontology" }
kg-storage = { path = "crates/kg-storage" }
kg-embeddings = { path = "crates/kg-embeddings" }
kg-algorithms = { path = "crates/kg-algorithms" }
kg-pipeline = { path = "crates/kg-pipeline" }
# Project-specific
da-domain = { path = "crates/da-domain" }
# …
```

Each `kg-*` crate is independently versionable and publishable to
crates.io. The `da-*` crates stay workspace-local.

### Reuse story

A new project (e.g. `reactivegraph`) adds to its `Cargo.toml`:

```toml
[dependencies]
kg-ontology = "0.1"
kg-storage = { version = "0.1", features = ["samyama"] }
kg-embeddings = { version = "0.1", features = ["fd-api"] }
kg-algorithms = "0.1"
kg-pipeline = "0.1"
```

Supplies its own `data/ontology/*.yaml` (different node types — maybe
`SourceFile`, `Function`, `Class` instead of `Paper`/`Claim`), its own
adapters (Git instead of GROBID), and its own use cases. The kg-*
crates provide: schema loading, validation, storage, embeddings,
bi-temporal queries, causal chains, conflict detection, decision
records, pipeline orchestration. Zero daily-archive code pulled in.

### Compatibility with existing ADRs

| ADR | Status after this decision |
|-----|---------------------------|
| ADR-040 (Samyama sole store) | Unchanged. kg-storage wraps Samyama. |
| ADR-044 (schema lifecycle) | schema_version field stays; now references YAML version. |
| ADR-045 (validator) | Subsumed by kg-ontology. NodeSchemaDef trait deprecated. |
| ADR-046 (bi-temporal) | Helpers move to kg-algorithms::temporal. Data (which fields are bi-temporal) in YAML aspects. |
| ADR-047 (conflicts) | Logic moves to kg-algorithms::conflict. Kinds/strategies in YAML dictionaries. |
| ADR-048 (decisions) | Logic moves to kg-algorithms::decision. Categories in YAML. |
| ADR-049 (pipeline DSL) | Moves wholesale to kg-pipeline. |

### Migration path (6 phases)

Each phase is independently shippable. No big-bang rewrite.

#### Phase A — kg-ontology skeleton (1-2 sessions)

- Create `crates/kg-ontology/` with module shells + Cargo.toml.
- Move `da-domain::validator` types (`Violation`, `Severity`,
  `PropertySnapshot`) into it.
- Move `da-domain::temporal` helpers into it (temporarily, will move
  again to kg-algorithms in Phase E).
- `da-domain` re-exports kg-ontology types for backward compat.
- No YAML loader yet — registry built from existing `all_node_schemas()`.
- All tests pass.

#### Phase B — kg-storage skeleton (1-2 sessions)

- Create `crates/kg-storage/`.
- Move `da-ports::graph_store` trait + types into it.
- Move `da-adapters::samyama_graph` adapter into it.
- Move `da-application/tests/common/mock_graph_store.rs` into it as
  `MockGraphStore` (the shared, label-aware, Clone-able mock).
- `da-ports` and `da-adapters` re-export for backward compat.
- All tests pass.

#### Phase C — kg-embeddings skeleton (1 session)

- Create `crates/kg-embeddings/`.
- Move `da-ports::embedder` trait into it.
- Move `da-adapters::fd_embedder` into it.
- Add `ruvector` feature flag with a `RuVectorBridge` stub (full
  integration deferred to a follow-up).
- All tests pass.

#### Phase D — YAML ontology data (2-3 sessions)

- Export all 31 existing `XSchema` structs to `data/ontology/*.yaml`
  via a `da ontology export` CLI command.
- Implement `OntologyRegistry::load_from_dir()` in kg-ontology.
- Validator reads from the registry instead of `all_node_schemas()`.
- `all_node_schemas()` becomes a deprecated adapter that delegates to
  the registry.
- Tests pass; YAML is now the source of truth for schema data.

#### Phase E — kg-algorithms + kg-pipeline (3-4 sessions)

- Create `crates/kg-algorithms/`.
- Move temporal helpers from kg-ontology to kg-algorithms::temporal.
- Move healing logic, conflict logic, decision logic from
  da-application to kg-algorithms (the project-specific parts — which
  entity types to scan — stay in da-application; the generic algorithms
  move).
- Create `crates/kg-pipeline/` with the DSL engine (ADR-049).
- All tests pass.

#### Phase F — deprecation cleanup (1-2 sessions)

- Mark `NodeSchemaDef` trait + 31 `XSchema` structs as `#[deprecated]`.
- Update all call sites to use `OntologyRegistry` directly.
- Remove deprecated items after a burn period.
- Publish kg-* crates (version 0.1.0) — internal milestone, not
  crates.io yet unless explicitly wanted.

#### Phase G — cross-project reuse validation (1 session)

- Add `kg-ontology`, `kg-storage` as dependencies of `reactivegraph`
  (separate repo).
- Supply a minimal `data/ontology/` with one or two node types.
- Confirm the validator + storage work end-to-end in a different
  project.
- Document the reuse story in each kg-* crate's README.

### Risks

1. **Compile-time field-name checking lost.** Typo in a field name
   surfaces at runtime (validator) instead of compile time. Mitigation:
   pre-commit `da ontology validate` + integration tests + a
   `da ontology check-rust` command that scans da-domain typed accessors
   against the loaded YAML.

2. **Migration churn.** Moving 31 schemas + 21 edge constants + 9
   cross-references + validators + helpers across crates is mechanical
   but voluminous. Mitigation: 6 phases, each independently shippable,
   each with a "all tests pass" gate.

3. **Cross-crate API churn during migration.** kg-ontology::validate
   vs da-domain::validator::validate during Phase A. Mitigation:
   re-exports in da-domain preserve the old paths; deprecation warnings
   guide consumers to the new locations.

4. **YAML schema-of-schemas needed.** The ontology YAML itself needs
   validation (a malformed `node_types.yaml` should fail loudly).
   Mitigation: strict serde deserialization + a `validate_ontology()`
   test + Phase D includes a meta-YAML that validates the ontology
   files.

5. **RuVector vendor coupling.** kg-embeddings pulls the RuVector
   vendor crate, which is heavy. Mitigation: feature flag
   `ruvector` (default off); consumers opt in.

## Alternatives considered

### 1. Single mega-crate `kg-core` with modules

Rejected. Pulls everything (ontology + storage + embeddings + algorithms
+ pipeline) into every consumer. No way to take just storage without
embeddings. Violates the "granular crates" preference confirmed with
the user.

### 2. Keep everything in da-domain, just move to YAML loading

Rejected. da-domain stays daily-archive-specific. Other projects would
depend on da-domain and pull in Paper/Claim/ResearchProblem. Violates
the reuse goal.

### 3. Adopt an existing Rust KG framework

No production-grade Rust KG framework exists as of 2026-07 that matches
our needs (property graph + YAML ontology + Samyama + RuVector). RDF
frameworks (sophia, rdf-rs) impose triple-store semantics we don't want.
Building our own kg-* family is the right call.

### 4. Use sub-crates under da-* names (da-ontology, da-storage)

Rejected. The `da-` prefix ties them to daily-archive conceptually even
if they have no da-* deps. The `kg-` prefix signals neutrality and
makes the reuse boundary obvious to anyone reading a Cargo.toml.

## Consequences

### Positive

- **Five independently versionable, releasable crates.** Consumers pick
  what they need.
- **Clean reuse boundary.** `reactivegraph`, `law-nexus`, future Rust
  KG projects add `kg-*` deps and supply their own ontology YAML +
  domain types. Zero daily-archive coupling.
- **"Не хардкодим" honoured fully.** All schema data in YAML; Rust
  crates hold logic only.
- **Testability.** Each kg-* crate has its own test suite, fixtures,
  and CI gate. Easier to test in isolation than a monolith.
- **Compile times.** Changing kg-algorithms doesn't rebuild kg-storage.
- **Discoverability.** Crate names communicate responsibility
  immediately (`kg-ontology` vs `kg-storage`).

### Negative

- **Five new crates to maintain.** More Cargo.toml files, more
  version coordination. Mitigated by workspace-level versioning.
- **One-time migration cost.** Phases A-F total 8-13 sessions of
  mechanical refactoring. Each phase is independently shippable.
- **Loss of compile-time schema checking.** Mitigated by CI checks
  (ontology validate, check-rust).
- **Cross-crate API design.** Getting the public API of each crate
  right requires care. Documented in this ADR; refined per crate in
  follow-up design docs.

## Open questions

1. **Publishing.** Do we publish kg-* to crates.io, or keep them
   path-dependencies in a multi-project workspace? Defer to Phase G —
   internal reuse first, crates.io after API stabilises.
2. **Versioning cadence.** Do kg-* crates version together (unified
   0.1.0, 0.2.0) or independently? Tentative: unified until 1.0,
   independent after.
3. **Feature flags in kg-storage.** Should Samyama be a feature flag
   (default) or mandatory? Tentative: feature flag, so consumers can
   use kg-storage with only the Mock for testing.
4. **da-domain's long-term role.** After migration, da-domain holds
   only typed view structs. Should it merge into da-application?
   Tentative: keep separate for clarity (types vs behavior).
5. **kg-pipeline's Stage trait.** Where does the `Stage` trait live?
   Tentative: in kg-pipeline; daily-archive implements
   `IngestStage`, `ExtractStage` etc. in da-application.

## Phase 1 deliverable (next session)

- Create `crates/kg-ontology/` with Cargo.toml + empty module shells
  matching the public API in this ADR.
- Move `da-domain::validator` types (Violation, Severity,
  PropertySnapshot) into kg-ontology.
- Move `da-domain::temporal` module into kg-ontology (temporary —
  will move to kg-algorithms in Phase E).
- `da-domain` re-exports kg-ontology types.
- Tests pass; CI green.
- This ADR marked **Accepted** in ADR-INDEX.md.

Full migration (Phases B-G) tracked as follow-up waves.
