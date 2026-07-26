# Graph Schema Design

**Status:** Binding (D132)
**Date:** 2026-07-26
**Implements:** ADR-040 §11 (schema-as-code), ADR-038 (entity/relation types)

This document is the **single source of truth** for the daily-archive v2
knowledge graph schema. All node types, edge types, properties, and indexes
are defined here. Loading code (ingest, extract) MUST validate against this
schema before writing. The `da schema init` command creates all indexes.

---

## Design principles

1. **Schema-as-code** (ADR-040 §11): schema lives in Rust types (`NodeSchemaDef`),
   not in Cypher DDL. DDL is generated from the types.
2. **VID is the identity** (ADR-037 §5): every node has a `vid` (SHA256-derived).
   Uniqueness is enforced by index + idempotent creation.
3. **Temporality** (ADR-037 §6): every node has `valid_from`; superseded nodes
   get `valid_to` + `superseded_by`. Not Samyama MVCC alone.
4. **Fail-closed import** (D127): `import_eligible=false` on every node until
   explicit human go. `evidence_ready=false` until evidence is grounded.
5. **Two edge families** (ADR-038 + bibliographic):
   - **Bibliographic** (structural metadata): CITES, MENTIONS, AUTHORED, HAS_SECTION
   - **Extracted** (semantic, 18 types): BuildsOn, UsesComponent, ... (Phase 3+)

---

## Node types

### 1. Paper

A scientific paper (the primary source node).

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | SHA256 VID (`vid:paper:<arxiv_id>`) |
| `arxiv_id` | String | ✅ | arXiv identifier |
| `title` | String | ✅ | Paper title |
| `abstract_text` | String | | Abstract (embedded for search) |
| `doi` | String | | DOI if available |
| `pdf_hash` | String | | SHA256 of source PDF |
| `section_count` | Integer | | Number of parsed sections |
| `citation_count` | Integer | | Number of parsed citations |
| `valid_from` | DateTime | ✅ | Ingest timestamp |
| `valid_to` | DateTime | | Set when superseded |
| `superseded_by` | String | | VID of superseding version |
| `schema_version` | Integer | | Schema version (migration) |
| `evidence_ready` | Boolean | | Has grounded evidence |
| `import_eligible` | Boolean | | D127 — always false until human go |
| `embedding` | Vector(1024) | | bge-m3 abstract embedding |

**Indexes:** `vid` (unique), `arxiv_id`, `embedding` (vector, cosine)

### 2. Citation

A cited reference (from GROBID parsed references).

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | SHA256 VID (`vid:paper:<arxiv_id>`) |
| `arxiv_id` | String | | arXiv id of cited paper (if resolvable) |
| `title` | String | | Title of cited work |
| `doi` | String | | DOI of cited work |
| `valid_from` | DateTime | ✅ | Creation timestamp |
| `schema_version` | Integer | | Schema version |

**Indexes:** `vid` (unique), `arxiv_id`

### 3. Entity

An extracted entity (ADR-038 Module B: Task, Method, Dataset, Model, Metric, Baseline).

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | SHA256 VID (`vid:entity:<type>:<label>`) |
| `label` | String | ✅ | Entity surface label |
| `entity_type` | String | ✅ | EntityType (Task/Method/Dataset/Model/Metric/Baseline) |
| `section` | String | | Section title where found |
| `char_start` | Integer | | Char offset in section text |
| `char_end` | Integer | | Char offset end |
| `surface` | String | | Exact surface text |
| `description` | String | | Optional description |
| `confidence` | Float | | Extraction confidence (0.0–1.0) |
| `valid_from` | DateTime | ✅ | Extraction timestamp |
| `schema_version` | Integer | | Schema version |
| `evidence_ready` | Boolean | | Has grounded evidence |
| `import_eligible` | Boolean | | D127 — always false |

**Indexes:** `vid` (unique), `entity_type`

### 4. Author (future)

A paper author. Not yet loaded, but schema defined for completeness.

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | SHA256 VID (`vid:author:<name>`) |
| `name` | String | ✅ | Author name |
| `valid_from` | DateTime | ✅ | Creation timestamp |

**Indexes:** `vid` (unique), `name`

### 5. Evidence (future — Phase 3 Slice 3)

An evidence assertion linking a claim to an immutable source artifact.

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | Evidence ID |
| `claim` | String | ✅ | The claim being evidenced |
| `span_type` | String | ✅ | PageBbox / CharOnly / Tei |
| `page` | Integer | | Page number |
| `char_start` | Integer | | Char offset |
| `char_end` | Integer | | Char offset end |
| `artifact_hash` | String | ✅ | SHA256 of source artifact |
| `artifact_path` | String | ✅ | Path to artifact |
| `epistemic_status` | String | ✅ | Verified / Staged / Pending |
| `created_at` | DateTime | ✅ | Creation timestamp |

**Indexes:** `vid` (unique), `artifact_hash`

---

## Edge types

### Bibliographic (structural metadata)

| Edge | From → To | Description | Status |
|------|-----------|-------------|--------|
| `CITES` | Paper → Citation | Paper cites a reference | ✅ implemented |
| `MENTIONS` | Paper → Entity | Paper mentions an entity | ✅ implemented |
| `AUTHORED` | Author → Paper | Author wrote paper | future |
| `HAS_SECTION` | Paper → Section | Paper has a section | future |
| `HAS_EVIDENCE` | Entity → Evidence | Entity grounded by evidence | future |

### Extracted (semantic — ADR-038, 18 types)

| Group | Types |
|-------|-------|
| Controlled (6) | BuildsOn, UsesComponent, AlternativeTo, Solves, AppliedTo, Targets |
| Composition (5) | UsesTechnique, ConsistsOf, Implements, Combines, Requires |
| Methodological (4) | DerivedFrom, DiffersFrom, HasLimitation, AddressesProblem |
| Citation argumentative (3) | Supports, Contrasts, Extends |

All extracted edges: Entity → Entity, with `confidence` + `source_spans`.
**Status:** not yet extracted (Phase 3 Slice 4+).

---

## Indexes (complete list)

| Index | Type | On | Purpose |
|-------|------|----|---------| 
| `paper_vid` | property (unique) | Paper.vid | VID lookup, idempotent create |
| `paper_arxiv_id` | property | Paper.arxiv_id | arxiv_id lookup |
| `paper_embedding` | vector (cosine, 1024) | Paper.embedding | semantic search |
| `citation_vid` | property (unique) | Citation.vid | idempotent create |
| `citation_arxiv_id` | property | Citation.arxiv_id | citation lookup |
| `entity_vid` | property (unique) | Entity.vid | idempotent create |
| `entity_type` | property | Entity.entity_type | filter by type |

---

## Schema version

Current: `1`. Increment when schema changes require migration.
Migration framework: ADR-040 §11.3 (future).

---

## Loading contract

1. **`da schema init`** — create all indexes BEFORE any data load.
2. **Ingest** — validate Paper + Citation properties against schema, then write.
3. **Extract** — validate Entity properties against schema, then write.
4. **Idempotent** — `find_node_by_string_property` before `create_node`.
5. **Fail-closed** — `import_eligible=false`, `evidence_ready=false` on every node.
