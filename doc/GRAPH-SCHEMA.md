# Graph Schema Design

**Status:** Binding (D132, revised)
**Date:** 2026-07-26
**Implements:** ADR-040 §11 (schema-as-code), ADR-038 (entity/relation types),
legacy ADR-028 (typed knowledge schema), legacy CanonicalDocument IR.

This document is the **single source of truth** for the daily-archive v2
knowledge graph schema. It captures the full article structure ("обвязка
статьи"): topics, keywords, sections, categories, authors, citations,
entities, and evidence. Loading code MUST validate against this schema.

---

## Design principles

1. **Article-first**: the graph models the article's own structure (sections,
   keywords, topics, categories) BEFORE extracted entities. The article is
   the spine; entities hang off it.
2. **Schema-as-code** (ADR-040 §11): schema lives in Rust types, not DDL.
3. **VID is identity** (ADR-037 §5): every node has a `vid` (SHA256-derived).
4. **Temporality** (ADR-037 §6): `valid_from` / `valid_to` / `superseded_by`.
5. **Fail-closed import** (D127): `import_eligible=false` until human go.
6. **Two edge families**: bibliographic (structural) + extracted (semantic).

---

## Node types

### Article spine (the "обвязка")

#### 1. Paper
A scientific paper — the root of the article subgraph.

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:paper:<arxiv_id>` |
| `arxiv_id` | String | ✅ | arXiv identifier |
| `title` | String | ✅ | Paper title |
| `abstract_text` | String | | Abstract |
| `doi` | String | | DOI |
| `pdf_hash` | String | | SHA256 of source PDF |
| `primary_category` | String | | arXiv primary category (e.g. cs.CL) |
| `published_at` | DateTime | | Publication date |
| `ingested_at` | DateTime | | Ingest timestamp |
| `section_count` | Integer | | Parsed section count |
| `citation_count` | Integer | | Parsed citation count |
| `keyword_count` | Integer | | Extracted keyword count |
| `valid_from` | DateTime | ✅ | Ingest timestamp |
| `valid_to` | DateTime | | Set when superseded |
| `schema_version` | Integer | | Schema version |
| `evidence_ready` | Boolean | | Has grounded evidence |
| `import_eligible` | Boolean | | D127 — always false |
| `embedding` | Vector(1024) | | bge-m3 abstract embedding |

**Indexes:** `vid` (unique), `arxiv_id`, `primary_category`, `embedding` (vector)

#### 2. Section
A structural section of a paper (from GROBID TEI `<div><head>`).

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:section:<paper_id>:<order>` |
| `title` | String | ✅ | Section heading |
| `level` | Integer | ✅ | Heading level (1, 2, 3...) |
| `order` | Integer | ✅ | Position in document |
| `text` | String | | Section body text |
| `char_count` | Integer | | Text length |
| `paper_id` | String | ✅ | Parent paper arxiv_id |

**Indexes:** `vid` (unique), `paper_id`
**Edges:** `HAS_SECTION` (Paper → Section)

#### 3. Keyword
A YAKE-extracted keyword from the paper.

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:keyword:<paper_id>:<normalized>` |
| `keyword` | String | ✅ | Keyword text |
| `score` | Float | ✅ | YAKE score (lower = better) |
| `language` | String | | Language code (en, etc.) |
| `paper_id` | String | ✅ | Parent paper |

**Indexes:** `vid` (unique), `keyword`
**Edges:** `HAS_KEYWORD` (Paper → Keyword)

#### 4. Topic
A research topic/theme the paper is about (derived from categories + keywords + title).

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:topic:<normalized>` |
| `label` | String | ✅ | Topic label (e.g. "prompt optimization") |
| `source` | String | ✅ | How derived: category / keyword / title |
| `confidence` | Float | | 0.0–1.0 |

**Indexes:** `vid` (unique), `label`
**Edges:** `ABOUT` (Paper → Topic)

#### 5. Category
An arXiv category (cs.CL, cs.CV, stat.ML, ...).

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:category:<code>` |
| `code` | String | ✅ | Category code (cs.CL) |
| `name` | String | | Human-readable name |
| `is_primary` | Boolean | ✅ | Primary category? |

**Indexes:** `vid` (unique), `code`
**Edges:** `IN_CATEGORY` (Paper → Category)

#### 6. Author
A paper author.

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:author:<name>` |
| `name` | String | ✅ | Author full name |
| `email` | String | | Email if available |
| `affiliation` | String | | Institution |

**Indexes:** `vid` (unique), `name`
**Edges:** `AUTHORED` (Author → Paper)

#### 7. Citation
A cited reference (from GROBID parsed references).

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:paper:<arxiv_id>` or `vid:citation:<hash>` |
| `arxiv_id` | String | | arXiv id if resolvable |
| `title` | String | | Title of cited work |
| `doi` | String | | DOI |
| `raw_text` | String | | Raw reference text |
| `valid_from` | DateTime | ✅ | Creation timestamp |
| `schema_version` | Integer | | Schema version |

**Indexes:** `vid` (unique), `arxiv_id`
**Edges:** `CITES` (Paper → Citation)

### Extracted content

#### 8. Entity
An extracted entity (ADR-038 Module B + legacy ADR-028 typed schema).

Entity types (closed vocabulary):
- **Concrete**: Method, Dataset, Metric, Task, Baseline, Model, Figure, Table,
  Equation, Concept, Implementation, Theorem, Definition
- **Abstract**: Problem, Motivation, Gap, Contribution, Hypothesis, Finding,
  Mechanism, Limitation, FutureWork

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:entity:<type>:<label>` |
| `label` | String | ✅ | Surface label |
| `entity_type` | String | ✅ | EntityType (closed vocabulary above) |
| `section_vid` | String | | Section where found |
| `char_start` | Integer | | Char offset in section |
| `char_end` | Integer | | Char offset end |
| `surface` | String | | Exact surface text |
| `description` | String | | Optional description |
| `confidence` | Float | | Extraction confidence |
| `valid_from` | DateTime | ✅ | Extraction timestamp |
| `schema_version` | Integer | | Schema version |
| `evidence_ready` | Boolean | | Has grounded evidence |
| `import_eligible` | Boolean | | D127 — always false |

**Indexes:** `vid` (unique), `entity_type`
**Edges:** `MENTIONS` (Paper → Entity), `MENTIONS_IN_SECTION` (Section → Entity)

#### 9. Evidence (future — Phase 3 Slice 3)
An evidence assertion linking a claim to an immutable source artifact.

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | Evidence ID |
| `claim` | String | ✅ | The claim |
| `span_type` | String | ✅ | PageBbox / CharOnly / Tei |
| `page` | Integer | | Page number |
| `char_start` | Integer | | Char offset |
| `char_end` | Integer | | Char offset end |
| `artifact_hash` | String | ✅ | SHA256 of artifact |
| `artifact_path` | String | ✅ | Path to artifact |
| `epistemic_status` | String | ✅ | Verified / Staged / Pending |
| `created_at` | DateTime | ✅ | Creation timestamp |

**Indexes:** `vid` (unique), `artifact_hash`
**Edges:** `HAS_EVIDENCE` (Entity → Evidence)

---

## Edge types

### Bibliographic / structural (article spine)

| Edge | From → To | Description | Status |
|------|-----------|-------------|--------|
| `HAS_SECTION` | Paper → Section | Paper has a section | schema only |
| `HAS_KEYWORD` | Paper → Keyword | Paper has a keyword | schema only |
| `ABOUT` | Paper → Topic | Paper is about a topic | schema only |
| `IN_CATEGORY` | Paper → Category | Paper in arXiv category | schema only |
| `AUTHORED` | Author → Paper | Author wrote paper | schema only |
| `CITES` | Paper → Citation | Paper cites a reference | ✅ implemented |
| `MENTIONS` | Paper → Entity | Paper mentions an entity | ✅ implemented |
| `MENTIONS_IN_SECTION` | Section → Entity | Entity found in section | schema only |
| `HAS_EVIDENCE` | Entity → Evidence | Entity grounded by evidence | future |

### Extracted / semantic (ADR-038 + legacy ADR-028, 27 types)

| Group | Types |
|-------|-------|
| Controlled (6) | BuildsOn, UsesComponent, AlternativeTo, Solves, AppliedTo, Targets |
| Causal (5) | Causes, Enables, Inhibits, Modulates, CorrelatedWith |
| Composition (5) | UsesTechnique, ConsistsOf, Implements, Combines, Requires |
| Comparison (7) | DerivedFrom, DiffersFrom, HasLimitation, AddressesProblem, MotivatedBy, HasProperty, SubsetOf |
| Citation argumentative (3) | Supports, Contrasts, Extends |

All extracted edges: Entity → Entity, with `confidence` + `source_spans`.
**Status:** not yet extracted (Phase 3 Slice 4+).

---

## Indexes (complete list)

| Index | Type | On | Purpose |
|-------|------|----|---------|
| `paper_vid` | property (unique) | Paper.vid | VID lookup |
| `paper_arxiv_id` | property | Paper.arxiv_id | arxiv lookup |
| `paper_category` | property | Paper.primary_category | category filter |
| `paper_embedding` | vector (cosine, 1024) | Paper.embedding | semantic search |
| `section_vid` | property (unique) | Section.vid | section lookup |
| `section_paper` | property | Section.paper_id | sections by paper |
| `keyword_vid` | property (unique) | Keyword.vid | keyword lookup |
| `keyword_text` | property | Keyword.keyword | keyword search |
| `topic_vid` | property (unique) | Topic.vid | topic lookup |
| `topic_label` | property | Topic.label | topic search |
| `category_vid` | property (unique) | Category.vid | category lookup |
| `category_code` | property | Category.code | category by code |
| `author_vid` | property (unique) | Author.vid | author lookup |
| `author_name` | property | Author.name | author search |
| `citation_vid` | property (unique) | Citation.vid | citation lookup |
| `citation_arxiv_id` | property | Citation.arxiv_id | citation by arxiv |
| `entity_vid` | property (unique) | Entity.vid | entity lookup |
| `entity_type` | property | Entity.entity_type | filter by type |

---

## Schema version

Current: `1`. Increment when schema changes require migration.

---

## Loading contract

1. **`da schema init`** — create all indexes BEFORE any data load.
2. **Ingest** — write Paper + Section + Keyword + Category + Author + Citation
   nodes (the full article spine), validate against schema.
3. **Extract** — write Entity nodes, link to Paper (MENTIONS) and Section
   (MENTIONS_IN_SECTION), validate against schema.
4. **Idempotent** — `find_node_by_string_property` before `create_node`.
5. **Fail-closed** — `import_eligible=false`, `evidence_ready=false`.
