# Graph Schema Lifecycle — External Storage, Versioning, Migration, Healing

**Status:** Design (2026-07-29)
**Related:** ADR-040 (Samyama sole store), ADR-042/043 (process plane), META-OPTIMIZATION-PLAN

## 1. Current state

### Что есть сейчас
- **Schema-as-code**: `NodeSchemaDef` trait + `all_node_schemas()` в da-domain
- **Graph indexes**: 20 hardcoded `CREATE INDEX` строк в da-graph/schema.rs
- **Schema version**: `CURRENT_SCHEMA_VERSION = 1` (одно число, не настоящее versioning)
- **Healing**: `GraphHealingUseCase` с 7 операциями (correct, silence, merge, split, etc.)
- **Snapshot**: `export_snapshot`/`import_snapshot` в port trait (backup mechanism)

### Чего нет
- **Schema registry**: schema definitions не хранятся в графе как nodes
- **Schema versioning**: нет semver, нет migration scripts, нет rollback
- **Schema migration framework**: нет expand-contract pattern, нет migration runner
- **Schema validation at write**: nodes создаются без проверки соответствия schema
- **Schema evolution tracking**: нет истории изменений schema
- **Schema healing**: healing работает на data level, не на schema level

## 2. Best practices 2026 (research summary)

### Ключевые принципы из research

1. **Schema as versioned contract** (graph-data-modeling.org)
   - Semver для schema: additive → minor bump, breaking → major bump
   - Schema manifest как артифакт под version control
   - Migration scripts как код, хранимые рядом со schema definitions

2. **Additive before destructive** (universal pattern)
   - Сначала добавить новое label/property/edge type
   - Затем dual-write период (старое + новое)
   - Затем backfill существующих данных
   - Затем переключить reads на новое
   - Затем удалить старое (после verification gate)

3. **Expand-Contract pattern** (zero-downtime migrations)
   - Phase 1 EXPAND: добавить новую структуру, писать в оба места
   - Phase 2 MIGRATE: backfill данных, переключить reads
   - Phase 3 CONTRACT: удалить старую структуру

4. **Idempotent migrations** (critical invariant)
   - Каждый migration step safe to run twice
   - Guard writes с `WHERE NOT EXISTS` / `MERGE`
   - Migration runner tracks applied migrations в dedicated node/table

5. **Defensive schema design** (theneuralbase.com)
   - Schema = learner, не gatekeeper
   - Start permissive → monitor → tighten после стабилизации pattern
   - Constraints описывают proven patterns, не предсказания

6. **Schema registry node** (Geode, TypeGraph)
   - Dedicated node label: `SchemaVersion` в самом графе
   - Хранит: version, description, timestamp, status (active/deprecated)
   - Позволяет rollback: переключить active version pointer

### Anti-patterns (из research)
- Big-bang cutover без dual-write window
- Non-idempotent backfill (double-create на retry)
- Encoding version как label (`:v1`, `:v2`) — фрагментирует planner statistics
- Destructive `DELETE` без verification gate
- Constraint добавленный слишком рано → блокирует real data

## 3. Как Samyama обрабатывает schema сейчас

### Что Samyama делает
- **Schemaless by default**: labels и edge types создаются on-the-fly при `create_node`/`create_edge`
- **Label index**: `HashMap<Label, HashSet<NodeId>>` — автоматически поддерживается
- **Property index**: `HashMap<(Label, PropertyName), BTreeMap<...>>` — создается явно
- **GraphCatalog**: triple-level statistics для cost-based optimization
- **RDFS reasoner**: schema-level reasoning (subClassOf, subPropertyOf)

### Чего Samyama НЕ делает
- Не имеет schema registry (нет ConceptNode для schema versions)
- Не имеет migration framework
- Не имеет schema validation at write
- Не имеет schema versioning
- Не имеет expand-contract migrations

**Вывод:** Samyama — schemaless graph engine. Schema management должен быть на уровне application/domain.

## 4. Как RuVector обрабатывает schema

### Что RuVector делает
- **Opt-in schema-first layer** (HelixDB-inspired, ADR-252)
  - Schemaless по умолчанию
  - Opt-in schema: объявленные labels/edges type-checked, undeclared pass-through
  - Vector types bound to node label + property (vector hit → graph traversal)
- **PropertyType**: Boolean, Integer, Float, String, Vector, Array, Map, Any
- **DistanceMetric**: Cosine, DotProduct, Euclidean

### Чего RuVector НЕ делает для нашего use case
- RuVector migration.rs — это agent migration protocol (WASM agents между partition)
- Не graph schema migration

**Вывод:** RuVector имеет opt-in schema validation, но не schema lifecycle management.

## 5. Предлагаемая архитектура

### Принцип: schema = first-class graph citizen

Schema не просто код в Rust — она **материализована в графе** как nodes, с версионированием и миграциями.

### Новые node types (Layer 1 — Schema Metadata)

```text
SchemaVersion
  vid: vid:schema:<version>
  version: "1.0.0"           (semver)
  description: "Initial schema"
  applied_at: timestamp
  status: active | deprecated | superseded
  migration_hash: SHA256
  retrieval_eligible: false   (schema metadata, не content)

SchemaMigration
  vid: vid:migration:<from>_<to>
  from_version: "1.0.0"
  to_version: "1.1.0"
  description: "Add EvidenceBundle + Claim node types"
  applied_at: timestamp
  status: pending | running | completed | failed | rolled_back
  operations: [operation descriptors]
```

### Архитектурные слои (hexagonal placement)

```text
┌─────────────────────────────────────────────────────────┐
│ da-domain                                                │
│   SchemaVersion, SchemaMigration types                  │
│   SchemaManifest (versioned schema definition)           │
│   SchemaDiff (from → to comparison)                     │
│   SchemaRegistry trait (port interface)                 │
│   MigrationOperation enum (add_type, add_property, ...)  │
└──────────────────────┬──────────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────────┐
│ da-ports                                                │
│   SchemaRegistry: Send + Sync trait                     │
│     fn get_active_version() -> SchemaVersion            │
│     fn get_manifest(version) -> SchemaManifest          │
│     fn apply_migration(migration) -> Result             │
│     fn rollback_migration(version) -> Result            │
│   SchemaValidator: Send + Sync trait                    │
│     fn validate_node(label, props) -> ValidationResult  │
│     fn validate_edge(edge_type) -> ValidationResult     │
└──────────────────────┬──────────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────────┐
│ da-application                                          │
│   SchemaMigrationUseCase                               │
│     fn plan_migration(from, to) -> MigrationPlan        │
│     fn execute_migration(plan) -> MigrationResult       │
│     fn verify_migration(version) -> VerificationResult  │
│   SchemaHealingUseCase (расширение GraphHealingUseCase) │
│     fn detect_schema_drift() -> Vec<SchemaDrift>        │
│     fn heal_schema_drift(drifts) -> HealingResult       │
└──────────────────────┬──────────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────────┐
│ da-adapters                                             │
│   SamyamaSchemaRegistry (impl SchemaRegistry)           │
│     SchemaVersion nodes in Samyama graph                │
│     SchemaMigration nodes in Samyama graph              │
│     Active version pointer in Samyama                   │
│   YamlSchemaManifestLoader                             │
│     Loads schema definitions from data/schema/*.yaml    │
└─────────────────────────────────────────────────────────┘
```

### External schema storage: `data/schema/`

```text
data/schema/
  manifest.yaml              ← current schema version + manifest pointer
  versions/
    v1.0.0.yaml              ← initial schema (28 node types)
    v1.1.0.yaml              ← +EvidenceBundle/Claim (ADR-042)
    v1.2.0.yaml              ← +Research Process Plane (ADR-043)
  migrations/
    v1.0.0_to_v1.1.0.yaml    ← expand-contract migration plan
    v1.1.0_to_v1.2.0.yaml    ← process plane migration
```

### manifest.yaml

```yaml
version: "1.2.0"
previous_version: "1.1.0"
description: "Research Process Plane (ADR-043)"
applied_at: "2026-07-29T12:00:00Z"
status: active

node_types:
  - label: Paper
    required: [vid, arxiv_id, title, valid_from]
    optional: [abstract_text, doi, scientific_domains, ...]
    indexes: [vid, arxiv_id, doi]
  - label: ResearchEnvironment
    required: [vid, completeness, research_problem_id, ...]
    optional: [environment_hash, compute_budget, ...]
    indexes: [vid]
  # ... 28 total

edge_types:
  - type: hasPart
    from: Work
    to: Section
    cardinality: one_to_many
  - type: MENTIONS
    from: Work
    to: Entity
    cardinality: many_to_many
  # ...

constraints:
  - type: unique_property
    label: Paper
    property: vid
  - type: required_property
    label: Paper
    property: arxiv_id
```

## 6. Migration lifecycle (expand-contract)

### Пример: добавление ResearchEnvironment (v1.1 → v1.2)

```text
Phase 1: EXPAND
  - Create SchemaVersion(1.2.0) node with status=pending
  - Add ResearchEnvironment node type (additive — safe)
  - Add MEMBER_OF_CLUSTER edge type (additive — safe)
  - Create indexes for new types
  - Dual-write: new ingest writes both old schema_version=1 AND new

Phase 2: MIGRATE
  - Backfill: для existing Entity nodes, create ResearchEnvironment
    где возможно (env_lite from paper metadata)
  - Verify: count nodes with schema_version=1.2
  - Verify: no orphaned entities without environment linkage

Phase 3: CONTRACT (после verification gate)
  - Mark SchemaVersion(1.1.0) as deprecated
  - Mark SchemaVersion(1.2.0) as active
  - Future ingest writes schema_version=1.2 only
```

### Migration script (YAML)

```yaml
# data/schema/migrations/v1.1.0_to_v1.2.0.yaml
from_version: "1.1.0"
to_version: "1.2.0"
description: "Research Process Plane (ADR-043)"
risk_level: low  # all additive changes

operations:
  - type: add_node_type
    label: ResearchEnvironment
    required: [vid, completeness, ...]
    indexes: [vid]

  - type: add_edge_type
    edge_type: MEMBER_OF_CLUSTER
    from: Entity
    to: ConceptCluster

  - type: backfill
    description: "Create env_lite for existing papers"
    query: |
      MATCH (p:Paper) WHERE NOT (p)--(:ResearchEnvironment)
      // ... create env_lite from paper metadata

verification:
  - type: count_nodes
    label: ResearchEnvironment
    expected_min: 0  # additive, may be empty initially

  - type: no_orphans
    description: "No Entity without MEMBER_OF_CLUSTER"
```

## 7. Schema healing (расширение существующего GraphHealingUseCase)

### Schema drift detection

```text
SchemaDrift
  type: missing_index | orphaned_node | type_mismatch |
        missing_required_property | deprecated_label
  node_id: u64
  expected: String (per schema version)
  actual: String
  severity: low | medium | high
```

### Healing operations (расширение существующих 7)

```text
8. SCHEMA_MIGRATE_NODE
   Обновляет node properties до новой schema version
   (add missing required properties with defaults)

9. SCHEMA_DEPRECATE_LABEL
   Помечает старый label как deprecated (soft delete)

10. SCHEMA_REINDEX
    Пересоздает indexes после schema change

11. SCHEMA_VALIDATION_PASS
    Запускает full schema validation pass, создаёт
    SchemaValidationReport node с результатами
```

## 8. CLI commands

```bash
# Schema management
da schema init                     # Create indexes (existing)
da schema version                  # Show active schema version
da schema history                  # List all SchemaVersion nodes
da schema plan --to v1.3.0         # Dry-run migration plan
da schema migrate --to v1.3.0      # Execute migration
da schema verify                   # Verify schema consistency
da schema rollback --to v1.2.0     # Rollback to previous version
da schema drift                    # Detect schema drift
da schema heal                     # Auto-heal drift where safe
```

## 9. Security & safety

- **D127 applied**: SchemaVersion/SchemaMigration nodes имеют `import_eligible=false`
- **Migration verification gate**: destructive operations требуют explicit `--force` flag
- **Snapshot before migration**: `export_snapshot` перед каждой major migration
- **Idempotent operations**: все migration steps safe to run twice
- **Audit trail**: SchemaMigration nodes хранят полный history в графе

## 10. Priorities (waves)

### Wave A: Schema manifest externalization
- `data/schema/versions/v1.2.0.yaml` — export текущей schema из code
- `YamlSchemaManifestLoader` adapter
- SchemaManifest type в da-domain
- **Ценность:** schema visible как artifact, можно diff между версиями

### Wave B: Schema registry в графе
- SchemaVersion + SchemaMigration node types в da-domain
- SamyamaSchemaRegistry adapter в da-adapters
- `da schema version` / `da schema history` CLI commands
- **Ценность:** graph знает свою schema, audit trail

### Wave C: Migration framework
- SchemaMigrationUseCase в da-application
- MigrationOperation enum (add_type, add_property, reindex, etc.)
- Expand-contract pattern implementation
- `da schema plan` / `da schema migrate` CLI commands
- **Ценность:** safe schema evolution без ручных Cypher scripts

### Wave D: Schema validation at write
- SchemaValidator port trait
- SamyamaSchemaValidator adapter (uses Samyama label index)
- Validation в IngestUseCase/ExtractionUseCase перед write
- **Ценность:** fail-fast на schema violations, не silent corruption

### Wave E: Schema healing
- SchemaDrift detection
- SchemaHealingUseCase (расширение GraphHealingUseCase)
- `da schema drift` / `da schema heal` CLI commands
- **Ценность:** self-healing graph, detect & fix schema inconsistencies
