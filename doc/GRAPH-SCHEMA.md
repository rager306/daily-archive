# Graph Schema Design

**Status:** Binding (D132 + D133 — ontology-aligned three-layer architecture)
**Date:** 2026-07-26
**Alignment:** FaBiO (structural types) + CiTO (citation types) + OpenAlex (metadata backbone)
**Design doc:** doc/ONTOLOGY-ALIGNMENT.md

The graph uses a **three-layer hybrid architecture**:
1. **Metadata** (OpenAlex): Work, Author, Institution, Concept, Topic
2. **Structure** (GROBID/S2ORC): Section, Figure, Table, Equation
3. **Content** (extraction): Entity (Method, Dataset, ...), Relation

---

## Layer 0: Source Provenance (multi-source federation)

Tracks where data came from. Every Work links to exactly one Source via `FROM_SOURCE`.

### Source

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `vid` | String | ✅ | `vid:source:<code>` |
| `code` | String | ✅ | `arxiv` / `textbook` / `stanford` / `openalex` / `crossref` |
| `source_type` | String | ✅ | `pdf` / `html` / `markdown` / `api_json` |
| `domain` | String | ✅ | `scientific_paper` / `textbook` / `lecture_notes` / `code_repo` |
| `reliability_tier` | Integer | | 1=curated, 2=extracted, 3=user |
| `access_method` | String | | `grobid` / `html_parser` / `openalex_api` |
| `retrieval_eligible` | Boolean | | D134: on ALL nodes |

**Indexes:** `vid` (unique), `code`, `domain`

### FROM_SOURCE edge (Work → Source)

Every Work links to exactly one Source. Enables:
- `MATCH (w:Work)-[:FROM_SOURCE]->(s:Source {code:'arxiv'})` — filter by source
- `MATCH (w:Work)-[:FROM_SOURCE]->(s:Source {domain:'textbook'})` — filter by domain

**Implemented in:** `crates/da-domain/src/source.rs` (SourceSchema + constants)

---

## Layer 1: Metadata (OpenAlex backbone)

### Work (FaBiO: fabio:Article / fabio:Preprint)

A scientific work. Generalized from `Paper` to support papers, preprints,
textbooks, code repos, tech docs.

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:work:<arxiv_id>` |
| `arxiv_id` | String | | arXiv identifier |
| `doi` | String | | DOI |
| `openalex_id` | String | | OpenAlex Work ID (W...) |
| `title` | String | ✅ | Work title |
| `abstract_text` | String | | Abstract |
| `work_type` | String | ✅ | article / preprint / book / textbook / code_repo / tech_doc |
| `publication_date` | String | | YYYY-MM-DD |
| `primary_category` | String | | arXiv primary category |
| `oa_status` | String | | open / closed / green / gold |
| `pdf_hash` | String | | SHA256 of source PDF |
| `section_count` | Integer | | Parsed section count |
| `reference_count` | Integer | | Parsed citation count |
| `concept_count` | Integer | | OpenAlex concepts linked |
| `cited_by_count` | Integer | | OpenAlex citation count |
| `valid_from` | DateTime | ✅ | Ingest timestamp |
| `valid_to` | DateTime | | Set when superseded |
| `schema_version` | Integer | | Schema version |
| `evidence_ready` | Boolean | | Has grounded evidence |
| `import_eligible` | Boolean | | D127 — always false |
| `embedding` | Vector(1024) | | bge-m3 abstract embedding |

**Indexes:** `vid` (unique), `arxiv_id`, `doi`, `primary_category`, `embedding` (vector)
**Edges:** `authoredBy` (→ Author), `hasConcept` (→ Concept), `hasTopic` (→ Topic),
`hasPart` (→ Section), `cites` (→ Work via CiTO), `mentions` (→ Entity)

### Author (FaBiO: foaf:Person + pro:author)

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:author:<name>` or `vid:author:<orcid>` |
| `name` | String | ✅ | Display name |
| `orcid` | String | | ORCID iD |
| `openalex_id` | String | | OpenAlex Author ID (A...) |
| `works_count` | Integer | | Total works (OpenAlex) |

**Indexes:** `vid` (unique), `orcid`, `name`
**Edges:** `authoredBy` (Author → Work), `affiliatedWith` (→ Institution)

### Institution (FaBiO: foaf:Organization)

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:inst:<ror>` or `vid:inst:<name>` |
| `name` | String | ✅ | Institution name |
| `country` | String | | Country code |
| `ror` | String | | ROR ID |
| `openalex_id` | String | | OpenAlex Institution ID (I...) |

**Indexes:** `vid` (unique), `ror`, `name`
**Edges:** `affiliatedWith` (Author → Institution)

### Concept (SKOS: skos:Concept / OpenAlex Concept)

A research concept from the OpenAlex concept hierarchy (levels 0–4).

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:concept:<openalex_id>` |
| `label` | String | ✅ | Concept label |
| `level` | Integer | ✅ | Hierarchy level (0=root … 4=leaf) |
| `wikidata` | String | | Wikidata Q-ID |
| `openalex_id` | String | | OpenAlex Concept ID (C...) |
| `works_count` | Integer | | Works tagged with this concept |

**Indexes:** `vid` (unique), `label`, `openalex_id`
**Edges:** `hasConcept` (Work → Concept), `broader` (Concept → Concept), `narrower` (Concept → Concept)

### Topic (FaBiO: fabio:Subject / OpenAlex Topic)

An OpenAlex topic — a grouped concept cluster (domain → field → subfield → topic).

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:topic:<openalex_id>` |
| `label` | String | ✅ | Topic label |
| `domain` | String | | Top-level domain |
| `field` | String | | Field within domain |
| `subfield` | String | | Subfield within field |
| `openalex_id` | String | | OpenAlex Topic ID (T...) |

**Indexes:** `vid` (unique), `label`, `openalex_id`
**Edges:** `hasTopic` (Work → Topic)

---

## Layer 2: Structure (GROBID / S2ORC)

### Section (FaBiO: fabio:DocumentObject)

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:section:<work_id>:<order>` |
| `title` | String | ✅ | Section heading |
| `level` | Integer | ✅ | Heading depth (1, 2, 3...) |
| `order` | Integer | ✅ | Position in document |
| `text` | String | | Section body text |
| `char_count` | Integer | | Text length |
| `work_vid` | String | ✅ | Parent Work VID |

**Indexes:** `vid` (unique), `work_vid`
**Edges:** `hasPart` (Work → Section), `foundIn` (Entity → Section)

---

## Layer 3: Content (domain extraction)

### Entity (ADR-028 typed schema, NOT in FaBiO)

Entity types (21, closed vocabulary):
- **Concrete (12)**: Method, Dataset, Metric, Task, Model, Figure, Table, Equation, Concept, Implementation, Theorem, Definition
- **Abstract (9)**: Problem, Motivation, Gap, Contribution, Hypothesis, Finding, Mechanism, Limitation, FutureWork

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:entity:<type>:<label>` |
| `label` | String | ✅ | Surface label |
| `entity_type` | String | ✅ | EntityType (closed vocabulary) |
| `section_vid` | String | | Section where found |
| `char_start` | Integer | | Char offset |
| `char_end` | Integer | | Char offset end |
| `surface` | String | | Exact surface text |
| `description` | String | | Optional description |
| `confidence` | Float | | Extraction confidence |
| `embedding` | Vector(1024) | | bge-m3 label embedding (Phase 3 GNN) |
| `domain_tags` | String | | Cross-domain tags (e.g., "rl,nlp,gnn") |
| `valid_from` | DateTime | ✅ | Extraction timestamp |
| `schema_version` | Integer | | Schema version |
| `evidence_ready` | Boolean | | Has grounded evidence |
| `import_eligible` | Boolean | | D127 — always false |

**Indexes:** `vid` (unique), `entity_type`, `embedding` (vector)
**Edges:** `mentions` (Work → Entity, weight=1.0), `foundIn` (Entity → Section),
27 extracted relation types (Entity → Entity)

### Reference (FaBiO: fabio:BibliographicReference)

A citation entry from the reference list. May resolve to a Work (if OpenAlex
or arXiv match is found) or remain a stub.

| Property | Type | Required | Description |
|----------|------|:--------:|-------------|
| `vid` | String | ✅ | `vid:ref:<hash>` |
| `raw_text` | String | ✅ | Raw reference text |
| `arxiv_id` | String | | Resolved arXiv id |
| `doi` | String | | Resolved DOI |
| `title` | String | | Title of cited work |
| `resolved_work_vid` | String | | VID of resolved Work (if any) |
| `valid_from` | DateTime | ✅ | Creation timestamp |

**Indexes:** `vid` (unique), `arxiv_id`, `doi`
**Edges:** `cites` (Work → Reference/Work, with CiTO citation type)

---

## Edge types (complete)

### Layer 1: Metadata edges

| Edge | CiTO/FaBiO | From → To | Description |
|------|-----------|-----------|-------------|
| `authoredBy` | `pro:author` | Work → Author | Author wrote work |
| `affiliatedWith` | `schema:affiliation` | Author → Institution | Author at institution |
| `hasConcept` | `dcterms:subject` | Work → Concept | Work tagged with concept |
| `hasTopic` | `fabio:hasSubject` | Work → Topic | Work belongs to topic |
| `broader` | `skos:broader` | Concept → Concept | Broader concept (hierarchy) |
| `narrower` | `skos:narrower` | Concept → Concept | Narrower concept |
| `cites` | `cito:cites` | Work → Work/Reference | Citation (typed via property) |

### Layer 2: Structure edges

| Edge | FaBiO | From → To | Description |
|------|-------|-----------|-------------|
| `hasPart` | `frbr:part` | Work → Section | Work contains section |

### Layer 3: Content edges

| Edge | Source | From → To | Description | Weight |
|------|--------|-----------|-------------|--------|
| `mentions` | extraction | Work → Entity | Work mentions entity | 1.0 (rule-based confidence) |
| `foundIn` | extraction | Entity → Section | Entity in section | — |
| `BUILDS_ON` | ADR-028 | Entity → Entity | Entity builds on entity | — |
| `USES_METHOD_IN` | CiTO | Entity → Entity | Uses method from | — |
| ...25 more | ADR-028/CiTO | Entity → Entity | Typed relations | — |

**Edge weight property** (Phase 3 GNN readiness): `mentions` edges carry a
`weight` float property (default 1.0 for rule-based extraction). This enables
PPR (Personalized PageRank) and GNN message passing via RuVector Tier 2.

### CiTO citation typing (property on `cites` edge)

When a citation has a known context/intent, the `cites` edge carries a
`citation_type` property from CiTO:
`agreesWith`, `disagreesWith`, `discusses`, `extends`, `usesMethodIn`,
`usesDataFrom`, `obtainsBackgroundFrom`, `critiques`, `includesExcerptFrom`, etc.

---

## Layer 6: Evidence & Community (ADR-042 revised) + Research Process Plane (ADR-043)

### Research Process Plane (14 process kernel node types)

Cross-cutting plane distributed across L1–L6 (not Layer 8). See ADR-043
and PROCESS-SCHEMA-P0.md for full design.

| Node | Layer | Semantics |
|------|-------|----------|
| `ResearchProblem` | L1 | what must be improved or explained |
| `ResearchEnvironment` | L1/L2 | two-tier n-ary verification context (full/env_lite) |
| `BaselineSnapshot` | L2 | concrete baseline artifact+config |
| `ResearchIdea` | L6 | natural-language proposal (not necessarily testable) |
| `Hypothesis` | L6 | first-class pre-test expectation (≠ Claim) |
| `Intervention` | L3 | normalized change (method/arch/protocol/exposure/param) |
| `InterventionBundle` | L3 | compound recipe (avoids opaque Method) |
| `ImplementationAttempt` | L6 | idea → artifact attempt (≠ ExperimentRun) |
| `ArtifactVersion` | L2 | immutable code/config/model/container hash |
| `ExperimentRun` | L6 | execution of artifact in environment |
| `MetricDefinition` | L3 | metric name + protocol (reusable) |
| `MetricObservation` | L6 | raw measured value (≠ Comparison ≠ Claim) |
| `ResultComparison` | L6 | candidate vs baseline derived comparison |
| `FailureEvent` | L6 | structured non-execution cause (≠ refutation) |

**Invariants (normative, ADR-043):**
  ResearchIdea ≠ Hypothesis ≠ Claim
  ImplementationAttempt ≠ ExperimentRun
  FailureEvent ≠ NegativeResult (no REFUTES edge)
  MetricObservation ≠ ResultComparison ≠ Claim ≠ RewardSignal
  Experimental Claim requires VALID_UNDER → ResearchEnvironment
  Environment completeness explicit (full | env_lite | unknown)
  All process nodes: retrieval_eligible + import_eligible (D127 default false)

### Publication Evidence & Community

### SchedulerTask (Layer 1 — Operational State)

| Property | Type | Description |
|----------|------|-------------|
| `vid` | String | `vid:task:<id>` |
| `arxiv_id` | String | Paper being processed |
| `task_type` | String | `openalex_enrich` / `ingest` / `extract` |
| `status` | String | `pending` / `running` / `completed` / `failed` |
| `retry_count` | Integer | Exponential backoff retry count |
| `next_retry` | DateTime | Next retry timestamp |
| `retrieval_eligible` | Boolean | false (operational, not content) |
| `import_eligible` | Boolean | false (D127) |

### ConceptCluster (derived community)

| Property | Type | Description |
|----------|------|-------------|
| `vid` | String | `vid:hyper:<label>` |
| `label` | String | Human-readable name |
| `cluster_type` | String | `concept_cluster` / `method_family` / `benchmark_suite` |
| `embedding` | Vector(1024) | Aggregate embedding |
| `retrieval_eligible` | Boolean | D134 |

**NOT an evidence unit.** Derived community for retrieval expansion.

### EvidenceBundle (source-grounded n-ary evidence)

| Property | Type | Description |
|----------|------|-------------|
| `vid` | String | `vid:bundle:<id>` |
| `bundle_type` | String | `experiment_setup` / `result_bundle` / `citation_context` |
| `normalized_text` | String | Normalized evidence text |
| `source_span_id` | String | Grounded source span |
| `document_id` | String | Source paper VID |
| `extraction_confidence` | Float | Extraction confidence |
| `verification_status` | String | `pending` / `verified` / `disputed` |
| `retrieval_eligible` | Boolean | D134 |

### Claim (proposition-bearing)

| Property | Type | Description |
|----------|------|-------------|
| `vid` | String | `vid:claim:<id>` |
| `text` | String | Proposition text |
| `claim_type` | String | `factual` / `methodological` / `comparative` |
| `source_span_id` | String | Grounded source span |
| `retrieval_eligible` | Boolean | D134 |

### Evidence edges

| Edge | From → To | Description |
|------|-----------|-------------|
| `MEMBER_OF_CLUSTER` | Entity → ConceptCluster | Community membership (NOT evidence) |
| `PARTICIPATES_IN` | Entity → EvidenceBundle | Role-bearing n-ary participation |
| `SUPPORTS` | EvidenceBundle → Claim | Evidence supports claim |
| `CONTRADICTS` | EvidenceBundle → Claim | Evidence contradicts claim |
| `QUALIFIES` | EvidenceBundle → Claim | Partial/conditional support |

---

## Indexes (complete — 26 indexes)

| Index | Type | On |
|-------|------|----|
| `work_vid` | unique | Work.vid |
| `work_arxiv_id` | property | Work.arxiv_id |
| `work_embedding` | vector(1024) | Work.embedding |
| `citation_vid` | unique | Citation.vid |
| `citation_arxiv_id` | property | Citation.arxiv_id |
| `entity_vid` | unique | Entity.vid |
| `entity_type` | property | Entity.entity_type |
| `entity_embedding` | vector(1024) | Entity.embedding (Phase 3 GNN) |
| `section_vid` | unique | Section.vid |
| `section_work` | property | Section.work_vid |
| `keyword_vid` | unique | Keyword.vid |
| `keyword_text` | property | Keyword.text |
| `topic_vid` | unique | Topic.vid |
| `topic_label` | property | Topic.label |
| `category_vid` | unique | Category.vid |
| `category_code` | property | Category.code |
| `source_vid` | unique | Source.vid (Layer 0) |
| `source_code` | property | Source.code (Layer 0) |
| `cluster_vid` | unique | ConceptCluster.vid (Layer 6) |
| `cluster_type` | property | ConceptCluster.cluster_type (Layer 6) |

---

## Loading contract

1. **`da schema init`** — create all 26 indexes (9 node types + Source + Reference + Author + SchedulerTask +
   ConceptCluster + Entity vector).
2. **`da enrich --from openalex`** (Phase A) — fetch Work + Author + Institution +
   Concept + Topic from OpenAlex API. **Replaces** YAKE keywords, category
   guessing, author parsing, citation resolution.
3. **`da ingest`** — parse PDF via GROBID, write Section nodes (structure layer).
4. **`da extract`** — extract Entity nodes from sections (content layer).
5. **Idempotent** — `find_node_by_string_property` before create.
6. **Fail-closed** — `import_eligible=false`, `evidence_ready=false`.

---

## ADHD-derived patterns (D134)

Four patterns from systematic divergent ideation (doc/ADHD-ONTOLOGY-RESEARCH.md):

### 1. retrieval_eligible on ALL nodes
Every node has `retrieval_eligible: Boolean` (default `false`).
- `false`: deprecated Concepts, quarantined agent writes, unparsed spans
- `true`: promoted, live taxonomy nodes
- All retrieval paths (PPR, BM25+RRF, GNN-rerank, Cypher expand) MUST filter
  on `retrieval_eligible=true`. This is the "epigenetic silencing" pattern.

### 2. Topic assignment audit trail
Every `hasTopic` edge carries:
- `assignment_method`: citation_clustering | LLM_label | ML_classifier
- `hierarchy_snapshot_id`: points to frozen OpenAlex dump version
- `score`: confidence (0.0–1.0)
- `assigned_at`: timestamp

This makes Concept→Topic deprecation remaps reproducible and reversible.

### 3. Provenance ring (future)
Provenance metadata lives in an append-only `ProvenanceEvent` audit ring.
Nodes hold only a ring offset pointer, not inline provenance.
This keeps hot retrieval paths cache-clean.

### 4. Agent quarantine (future)
RuVector/SONA agent-memory writes land as `QuarantinedAssertion` edges with
`retrieval_eligible=false`. They cannot seed or expand in retrieval until a
`PromotionCertificate` (human go or automated ship-gate) flips them to live.
Two-lane: certified lane (users/PPR) vs quarantine lane (agent rehearsal).
