# ADR-044: Graph Schema Lifecycle — Versioned Manifest, Migration Framework, Self-Healing

**Status:** Proposed  
**Date:** 2026-07-29  
**Deciders:** collaborative  
**Related:** ADR-040 (Samyama sole store, schemaless engine), ADR-042 (EvidenceBundle/Claim), ADR-043 (Research Process Plane, 28 node types), D127 (fail-closed import), D132 (graph schema single source of truth), D134 (retrieval_eligible on ALL nodes)

## Context

daily-archive v2 has **28 node types** across Publication and Research Process planes, with schemas defined as Rust code (`NodeSchemaDef` trait + `all_node_schemas()`). The graph engine (Samyama) is **schemaless** — labels and edge types are created on-the-fly at write time. This creates several problems as the graph grows:

1. **No schema registry**: the graph does not know its own schema. There is no record of which node types, edge types, or indexes should exist.
2. **No versioning**: `CURRENT_SCHEMA_VERSION = 1` is a single integer, not semver. There is no migration history, no rollback capability, no audit trail.
3. **No migration framework**: schema changes (adding node types, renaming properties, adding indexes) require manual Cypher scripts with no idempotency guarantees or verification gates.
4. **No validation at write**: nodes are created without checking conformity to the declared schema. Silent schema drift accumulates.
5. **No schema healing**: the existing `GraphHealingUseCase` (7 operations) works on data-level issues (merge duplicates, correct properties) but not schema-level issues (missing indexes, orphaned labels, deprecated types).

Research into 2026 best practices (graph-data-modeling.org, TypeGraph, Geode DB, theneuralbase.com, GraFlo) consistently identifies:

- **Schema as versioned contract**: semver, schema manifest as artifact under version control.
- **Additive before destructive**: expand-contract pattern for zero-downtime migrations.
- **Idempotent migrations**: every step safe to run twice, guarded by existence checks.
- **Defensive schema design**: schema is a learner, not a gatekeeper — start permissive, tighten after observation.
- **Schema registry in-graph**: dedicated `SchemaVersion` nodes for audit trail and rollback.

Samyama analysis confirms: schemaless engine, no schema registry, no migration framework, no versioning. Schema management must be an **application-layer concern**, not delegated to the graph engine.

RuVector analysis: has opt-in schema-first layer (type checking for declared labels/edges), but its `migration.rs` is an agent migration protocol (WASM agents between partitions), not graph schema migration. No direct reuse.

## Decision

### 1. Schema as first-class graph citizen

Schema definitions are materialized in the graph as metadata nodes, with full versioning, migration history, and audit trail.

Two new node types (Layer 1 — Schema Metadata):

```text
SchemaVersion
  vid: vid:schema:<semver>
  version: "1.2.0"                    (semantic versioning)
  description: "Research Process Plane"
  applied_at: timestamp
  status: active | deprecated | superseded
  migration_hash: SHA256
  retrieval_eligible: false            (D134 — metadata, not content)
  import_eligible: false               (D127 — fail-closed)

SchemaMigration
  vid: vid:migration:<from>_<to>
  from_version: "1.1.0"
  to_version: "1.2.0"
  description: "Add ResearchEnvironment + process nodes"
  applied_at: timestamp
  status: pending | running | completed | failed | rolled_back
  operations: [migration step descriptors]
```

### 2. External schema manifest (YAML)

Schema definitions live as versioned YAML artifacts — single source of truth, diffable, version-controlled.

```text
data/schema/
  manifest.yaml                    ← active version pointer
  versions/
    v1.0.0.yaml                    ← initial schema (publication plane)
    v1.1.0.yaml                    ← +EvidenceBundle/Claim (ADR-042)
    v1.2.0.yaml                    ← +Research Process Plane (ADR-043)
  migrations/
    v1.0.0_to_v1.1.0.yaml          ← expand-contract plan
    v1.1.0_to_v1.2.0.yaml
```

Each version manifest declares: node types (required/optional fields, indexes), edge types (from/to/cardinality), and constraints (unique, required).

### 3. Hexagonal architecture placement

```text
da-domain
  SchemaVersionSchema, SchemaMigrationSchema (NodeSchemaDef)
  SchemaManifest (versioned schema definition struct)
  SchemaDiff (from → to comparison)
  MigrationOperation enum (add_type, add_property, reindex, ...)

da-ports
  SchemaRegistry trait: get_active_version, get_manifest, apply_migration, rollback
  SchemaValidator trait: validate_node, validate_edge

da-application
  SchemaMigrationUseCase: plan_migration, execute_migration, verify_migration
  SchemaHealingUseCase: detect_schema_drift, heal_schema_drift

da-adapters
  SamyamaSchemaRegistry: SchemaVersion nodes in Samyama graph
  YamlSchemaManifestLoader: loads from data/schema/*.yaml
```

### 4. Expand-Contract migration pattern

All breaking changes follow a three-phase zero-downtime migration:

**Phase 1 — EXPAND (additive, safe):**
- Create `SchemaVersion(target)` node with `status=pending`
- Add new node types, edge types, indexes (all additive)
- Dual-write: new writes include both old and new schema markers

**Phase 2 — MIGRATE (backfill + verify):**
- Backfill existing data to new structure
- Run verification queries (count, orphan check, type check)
- Gate: verification must pass before proceeding

**Phase 3 — CONTRACT (destructive, gated):**
- Mark `SchemaVersion(old)` as `deprecated`
- Mark `SchemaVersion(new)` as `active`
- Remove deprecated structures only after explicit `--force` flag
- Snapshot before destructive step (`export_snapshot`)

### 5. Idempotent migration operations

Every migration step is safe to run twice:
- Guard writes with `WHERE NOT EXISTS` / `MERGE` semantics
- Migration runner tracks applied operations in `SchemaMigration` node
- Re-running a completed migration is a no-op

### 6. Schema drift detection + healing

Extends the existing `GraphHealingUseCase` (7 operations) with 4 schema-specific operations:

```text
8. SCHEMA_MIGRATE_NODE     — update node properties to new schema version
9. SCHEMA_DEPRECATE_LABEL  — mark old label as deprecated (soft delete)
10. SCHEMA_REINDEX         — rebuild indexes after schema change
11. SCHEMA_VALIDATION_PASS — full validation, create report node
```

`SchemaDrift` detection identifies: missing indexes, orphaned nodes (label not in schema), type mismatches, missing required properties, deprecated labels still in use.

### 7. CLI commands

```bash
da schema init                  # Create indexes (existing)
da schema version               # Show active schema version
da schema history               # List all SchemaVersion nodes
da schema plan --to v1.3.0      # Dry-run migration plan
da schema migrate --to v1.3.0   # Execute migration
da schema verify                # Verify schema consistency
da schema rollback --to v1.2.0  # Rollback to previous version
da schema drift                 # Detect schema drift
da schema heal                  # Auto-heal drift where safe
```

### 8. Safety invariants

- **D127**: `SchemaVersion` and `SchemaMigration` nodes have `import_eligible=false`.
- **D134**: `retrieval_eligible=false` on schema metadata nodes (not content).
- **Verification gate**: destructive operations require explicit `--force` flag.
- **Snapshot before migration**: `export_snapshot` before every major migration.
- **Idempotent**: all migration steps safe to run twice.
- **Audit trail**: `SchemaMigration` nodes store full history in the graph.
- **No version labels**: store version as indexed property, not as `:v1` label (avoids planner statistics fragmentation).

## Consequences

### Positive

- Graph knows its own schema — enables self-description, validation, and tooling.
- Schema changes are safe, incremental, and auditable.
- Rollback capability via `SchemaVersion` active pointer switch.
- Schema drift detected and healed automatically.
- Schema manifest as YAML artifact enables CI/CD validation and diffing.
- Aligns with 2026 best practices (expand-contract, idempotent, defensive schema).

### Negative / costs

- Two new node types (`SchemaVersion`, `SchemaMigration`) — schema overhead.
- Migration framework complexity (migration runner, plan generator, verifier).
- YAML schema manifests must be kept in sync with Rust `NodeSchemaDef` code.
- Initial migration from code-only schema to manifest-based requires bootstrapping.

### Migration / implementation posture

Design-first, implement in five waves:

1. **Wave A**: Schema manifest externalization (`data/schema/versions/*.yaml` + loader).
2. **Wave B**: Schema registry in graph (`SchemaVersion` + `SchemaMigration` nodes + CLI).
3. **Wave C**: Migration framework (expand-contract, idempotent operations, plan/migrate commands).
4. **Wave D**: Schema validation at write (`SchemaValidator` in ingest/extract pipelines).
5. **Wave E**: Schema healing (drift detection + auto-heal + CLI commands).

## Compatibility

| Existing | Role under ADR-044 |
|----------|--------------------|
| `NodeSchemaDef` trait | Stays as Rust representation; YAML manifest is generated from it |
| `all_node_schemas()` | Used to bootstrap initial manifest export |
| `CURRENT_SCHEMA_VERSION` | Replaced by semver in `SchemaVersion` node |
| `GraphHealingUseCase` (7 ops) | Extended with 4 schema-specific operations |
| `export_snapshot`/`import_snapshot` | Used as pre-migration safety net |
| 20 hardcoded indexes in da-graph | Migrated to schema manifest, applied via migration runner |

## What we do NOT do

- Do NOT delegate schema management to Samyama (it is schemaless by design).
- Do NOT use version-encoded labels (`:v1`, `:v2`) — fragments planner statistics.
- Do NOT perform big-bang schema cutover without dual-write window.
- Do NOT add constraints before patterns are proven stable (defensive schema).
- Do NOT auto-delete deprecated structures without verification gate.
