# Ontology Design — Multi-Source, Multi-Domain Knowledge Graph

**Status:** Design (proposed)
**Date:** 2026-07-29
**Supersedes:** GRAPH-SCHEMA.md (extends, not replaces)
**Related:** ADR-037 (RuVector agent brain), ADR-040 (Samyama sole store), D133 (ontology alignment)

---

## 1. Vision

daily-archive — это **multi-source, multi-domain scientific knowledge engine**.
Онтология должна поддерживать:

1. **Multi-source federation** — arxiv, textbooks, Stanford, OpenAlex, Crossref, code repos
2. **Multi-domain profiles** — scientific paper, textbook chapter, lecture notes, code repository
3. **Subgraph extraction** — citation neighborhoods, topic clusters, method lineages, communities
4. **Hypergraph formation** — grouping entities into concept clusters, method families, benchmark suites
5. **Temporality** — versioned entities, point-in-time queries, change tracking
6. **Summary generation** — paper summaries, topic surveys, author profiles, method evolution
7. **GNN readiness** — embeddings on ALL nodes, typed adjacency, edge weights for PPR/message passing

---

## 2. Seven-Layer Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│ Layer 7: AGENT MEMORY (RuVector Tier 2, future)                      │
│   Nodes: AgentAssertion, LearnedPattern, CommunityCluster             │
│   Edges: ASSERTS, LEARNED_FROM, SIMILAR_TO (GNN-derived)              │
│   GNN: PPR rankings, community detection, node embeddings             │
├──────────────────────────────────────────────────────────────────────┤
│ Layer 6: HYPERGRAPH (concept aggregation)                             │
│   Nodes: ConceptCluster, MethodFamily, BenchmarkSuite                 │
│   Edges: MEMBER_OF, SUBSUMES, DERIVES_FROM                            │
│   Hyperedges group Layer 3 entities into higher-level concepts        │
├──────────────────────────────────────────────────────────────────────┤
│ Layer 5: TEMPORAL (versioning & change tracking)                      │
│   Properties on ALL nodes: valid_from, valid_to, superseded_by       │
│   Nodes: Snapshot, ProvenanceEvent                                     │
│   Edges: SUPERSEDES, REVERTS_TO, SNAPSHOT_OF                          │
├──────────────────────────────────────────────────────────────────────┤
│ Layer 4: RELATIONS (typed entity-entity)                              │
│   Edges: 27+ CiTO/ADR-028 types (BUILDS_ON, USES_METHOD_IN, etc.)    │
│   Edge properties: confidence, evidence_spans, citation_type          │
├──────────────────────────────────────────────────────────────────────┤
│ Layer 3: CONTENT (domain extraction)                                  │
│   Nodes: Entity (Method, Dataset, Model, Metric, Task, ...)           │
│   Source: RuleBasedExtractor / GLiNER (future)                        │
│   Edges: mentions, foundIn                                             │
├──────────────────────────────────────────────────────────────────────┤
│ Layer 2: STRUCTURE (document structure)                               │
│   Nodes: Section, Chapter, Figure, Table, Equation, CodeBlock        │
│   Source: GROBID TEI / HTML parser / Markdown parser                  │
│   Edges: hasPart, isPartOf, hasChapter                                 │
├──────────────────────────────────────────────────────────────────────┤
│ Layer 1: METADATA (curated bibliographic)                             │
│   Nodes: Work, Author, Institution, Topic, Source                     │
│   Source: OpenAlex API / arXiv / Crossref / manual                    │
│   Edges: authoredBy, hasTopic, cites, FROM_SOURCE                     │
├──────────────────────────────────────────────────────────────────────┤
│ Layer 0: SOURCE PROVENANCE (new)                                      │
│   Nodes: Source (arxiv, textbook, stanford, openalex, crossref)       │
│   Properties: type, domain, reliability_tier, access_method           │
│   Every Work links to its Source via FROM_SOURCE                      │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 3. Layer 0: Source Provenance (NEW)

Tracks **where data came from** — enables multi-source federation.

### Source node

| Property | Type | Description |
|----------|------|-------------|
| `vid` | String | `vid:source:<code>` |
| `code` | String | `arxiv` / `textbook` / `stanford` / `openalex` / `crossref` / `semantic_scholar` |
| `source_type` | String | `pdf` / `html` / `markdown` / `api_json` / `latex` |
| `domain` | String | `scientific_paper` / `textbook` / `lecture_notes` / `code_repo` / `blog` |
| `reliability_tier` | Integer | 1=curated (OpenAlex), 2=extracted (GROBID), 3=user-provided |
| `access_method` | String | `grobid` / `html_parser` / `openalex_api` / `crossref_api` |
| `base_url` | String | API base or website root |
| `retrieval_eligible` | Boolean | D134: on ALL nodes |

### FROM_SOURCE edge (Work → Source)

Every Work node links to exactly one Source. This enables:
- "Show all papers from arxiv" → `MATCH (w:Work)-[:FROM_SOURCE]->(s:Source {code:'arxiv'})`
- "Show all textbook chapters" → `MATCH (w:Work)-[:FROM_SOURCE]->(s:Source {domain:'textbook'})`
- "Which sources have reliability_tier=1?" → curated sources only

---

## 4. Layer 1: Metadata (ENHANCED)

### Work node (enhanced)

Existing fields + new:

| New Property | Type | Description |
|---|---|---|
| `domain_profile` | String | `paper` / `textbook` / `lecture` / `code` / `blog` |
| `source_code` | String | Links to Source.code (denormalized for fast filtering) |
| `summary` | String | Auto-generated one-paragraph summary (Layer 6) |
| `embedding` | Vector(1024) | bge-m3 embedding (existing, but now on ALL node types, not just Work) |

### Source-specific profiles

Each `domain_profile` activates different optional fields:

**PaperProfile** (domain_profile = "paper"):
- `doi`, `publication_date`, `oa_status`, `primary_category`
- `cited_by_count`, `reference_count`

**TextbookProfile** (domain_profile = "textbook"):
- `chapter_number`, `prerequisites`, `difficulty_level`
- `exercise_count`, `figure_count`

**LectureProfile** (domain_profile = "lecture"):
- `course_code`, `lecture_number`, `duration_minutes`
- `has_video`, `has_slides`, `has_transcript`

---

## 5. Layer 2: Structure (ENHANCED)

### New node types

| Node | Source | FaBiO | Description |
|------|--------|-------|-------------|
| Chapter | HTML/Markdown parser | `fabio:Chapter` | Textbook chapter (groups Sections) |
| CodeBlock | code parser | — | Code snippet with language + dependencies |
| Paragraph | GROBID/HTML | `fabio:Paragraph` | Atomic text unit (for fine-grained evidence) |

### New edges

| Edge | From → To | Description |
|------|-----------|-------------|
| `hasChapter` | Work → Chapter | Textbook has chapter |
| `hasParagraph` | Section → Paragraph | Section contains paragraph |
| `hasCodeBlock` | Section → CodeBlock | Section contains code |

---

## 6. Layer 3: Content (ENHANCED)

### Entity node (enhanced)

Existing fields + new:

| New Property | Type | Description |
|---|---|---|
| `embedding` | Vector(1024) | bge-m3 label embedding — enables GNN on entities |
| `domain_tags` | String[] | Cross-domain tags: `["rl", "nlp", "cv"]` |
| `mention_count` | Integer | How many Works mention this entity |
| `first_seen` | DateTime | First appearance in corpus |
| `popularity_trend` | Float | Derivative of mention_count over time |

### Key insight: embeddings on ALL nodes

Currently only Work nodes have embeddings. For GNN (RuVector Tier 2):
- **Entity embeddings**: `bge-m3("GPT-4 is a large language model by OpenAI")`
- **Section embeddings**: `bge-m3(section_text[:512])`
- **Author embeddings**: aggregate of work embeddings
- **Topic embeddings**: aggregate of entity embeddings in topic

This enables:
- GNN message passing across heterogeneous node types
- PPR from any node type (not just Work)
- Similarity search for entities, not just papers

---

## 7. Layer 4: Relations (ENHANCED)

### Edge weights (NEW)

Every edge gets a `weight` property for GNN/PPR:

| Edge type | Weight formula | Rationale |
|-----------|---------------|-----------|
| `cites` | 1.0 / (target_citation_count) | Popular papers have less influence per citation |
| `authoredBy` | 1.0 / sqrt(author_paper_count) | TF-IDF-like normalization |
| `mentions` | confidence score (0.0–1.0) | Extraction confidence |
| `hasTopic` | assignment_score (0.0–1.0) | OpenAlex topic assignment score |
| `BUILDS_ON` | 1.0 | Explicit relation |
| `SIMILAR_TO` | cosine_similarity | GNN-derived (future) |

### Typed adjacency for heterogeneous GNN

Each edge type becomes a separate adjacency matrix:
- A_cites[n×n] — citation graph
- A_mentions[n×m] — Work→Entity bipartite
- A_authoredBy[n×k] — Work→Author bipartite
- A_hasTopic[n×t] — Work→Topic bipartite

R-GCN / HGT can process these separately, then combine.

---

## 8. Layer 5: Temporal (NEW)

### Temporal properties (on ALL nodes)

Already partially implemented:
- `valid_from` — when node was created
- `valid_to` — when superseded (null = current)
- `superseded_by` — VID of replacement

### Snapshot nodes (NEW)

For point-in-time queries:

| Node | Description |
|------|-------------|
| `Snapshot` | Graph state at a timestamp (metadata only, not full copy) |
| `ProvenanceEvent` | Who/what/when changed a node (already exists in healing.rs) |

### Temporal query patterns

```cypher
// What did we know about PPO on 2026-01-01?
MATCH (e:Entity {label: "PPO"})
WHERE e.valid_from <= '2026-01-01' AND (e.valid_to IS NULL OR e.valid_to > '2026-01-01')
RETURN e

// All entity changes in last 7 days
MATCH (pe:ProvenanceEvent)
WHERE pe.timestamp > datetime() - duration('P7D')
RETURN pe

// State of graph before healing operation X
MATCH (s:Snapshot {healing_op_id: 'X'})
RETURN s
```

---

## 9. Layer 6: Hypergraph (IMPLEMENTED: ConceptCluster)

**Status:** ConceptClusterSchema implemented in `crates/da-domain/src/hypergraph.rs`.
MethodFamily and BenchmarkSuite use the same schema with different `cluster_type`.

### Hyperedge node types

| Node | Description | Example |
|------|-------------|---------|
| `ConceptCluster` | GNN-derived grouping of related entities | {PPO, DPO, GRPO, GEPA} → "RL optimization methods" |
| `MethodFamily` | Manually or semi-automatically defined method lineage | {BERT, RoBERTa, DeBERTa} → "encoder-only transformers" |
| `BenchmarkSuite` | Group of datasets used together for evaluation | {MMLU, BBH, ARC, HellaSwag} → "LLM evaluation suite" |

### Hyperedge properties

| Property | Type | Description |
|----------|------|-------------|
| `vid` | String | `vid:hyper:<label_slug>` |
| `label` | String | Human-readable name |
| `type` | String | `concept_cluster` / `method_family` / `benchmark_suite` |
| `description` | String | Auto-generated summary |
| `member_count` | Integer | Number of member entities |
| `embedding` | Vector(1024) | Aggregate embedding of members |
| `retrieval_eligible` | Boolean | D134 compliance |

### MEMBER_OF edge (Entity → Hyperedge)

| Property | Type | Description |
|----------|------|-------------|
| `weight` | Float | Membership strength (0.0–1.0) |
| `assignment_method` | String | `manual` / `gnn_cluster` / `co-occurrence` |
| `assigned_at` | DateTime | When membership was established |

### SUBSUMES edge (Hyperedge → Hyperedge)

Hyperedges can form hierarchies:
- "RL optimization methods" SUBSUMES "policy gradient methods"
- "policy gradient methods" SUBSUMES "PPO", "GRPO"

---

## 10. Layer 7: Agent Memory (FUTURE — RuVector Tier 2)

### AgentAssertion node

| Property | Type | Description |
|------|------|-------------|
| `vid` | String | `vid:assert:<uuid>` |
| `claim` | String | Natural language claim |
| `confidence` | Float | 0.0–1.0 |
| `evidence_vids` | String[] | VIDs of supporting evidence nodes |
| `status` | String | `quarantine` / `certified` / `rejected` |
| `embedding` | Vector(1024) | Claim embedding for GNN |

### SIMILAR_TO edge (GNN-derived)

Computed by RuVector GNN-rerank:
- Entity ↔ Entity (semantic similarity)
- Work ↔ Work (content similarity beyond citations)
- Author ↔ Author (research profile similarity)

| Property | Type | Description |
|----------|------|-------------|
| `score` | Float | Cosine similarity or GNN-predicted score |
| `method` | String | `cosine` / `ppr` / `gat` / `graphsage` |
| `computed_at` | DateTime | When similarity was last computed |

---

## 11. Subgraph Extraction Patterns

### Citation neighborhood (EXISTING)

```cypher
// 3-hop citation graph from paper X
MATCH (w:Work {vid: 'X'})-[:cites*1..3]->(cited:Work)
WHERE cited.retrieval_eligible = true
RETURN cited
```

### Topic cluster (NEW)

```cypher
// All entities + papers tagged with Topic "Reinforcement Learning"
MATCH (w:Work)-[:hasTopic]->(t:Topic {display_name: 'Reinforcement Learning'})
MATCH (w)-[:mentions]->(e:Entity)
RETURN w, e
```

### Method lineage (NEW)

```cypher
// All methods that BUILDS_ON PPO, recursively
MATCH (e:Entity {label: 'PPO'})<-[:BUILDS_ON*1..5]-(derivative:Entity)
RETURN derivative
```

### Community detection (FUTURE — RuVector GNN)

```cypher
// After GNN community detection assigns community IDs
MATCH (e:Entity {community_id: 'comm_42'})
RETURN e
```

### Cross-source subgraph (NEW)

```cypher
// All textbook chapters that mention entities also in arxiv papers
MATCH (chapter:Work)-[:FROM_SOURCE]->(s:Source {domain: 'textbook'})
MATCH (chapter)-[:mentions]->(e:Entity)<-[:mentions]-(paper:Work)-[:FROM_SOURCE]->(s2:Source {code: 'arxiv'})
RETURN chapter, e, paper
```

---

## 12. Summary Generation Patterns

### Paper summary

```cypher
MATCH (w:Work {vid: 'X'})
OPTIONAL MATCH (w)-[:mentions]->(e:Entity)
OPTIONAL MATCH (w)-[:hasTopic]->(t:Topic)
RETURN w.title, w.abstract_text, collect(DISTINCT e.label) as entities, collect(DISTINCT t.display_name) as topics
```

### Method evolution (temporal)

```cypher
// How PPO evolved over time
MATCH (e:Entity {label: 'PPO'})<-[:BUILDS_ON*0..3]-(derivative:Entity)
WHERE derivative.valid_from IS NOT NULL
ORDER BY derivative.valid_from
RETURN derivative.label, derivative.valid_from
```

### Author profile

```cypher
MATCH (a:Author)<-[:authoredBy]-(w:Work)
OPTIONAL MATCH (w)-[:mentions]->(e:Entity)
OPTIONAL MATCH (w)-[:hasTopic]->(t:Topic)
RETURN a.name, count(w) as papers, collect(DISTINCT e.label) as entities, collect(DISTINCT t.display_name) as topics
```

### Topic survey

```cypher
MATCH (w:Work)-[:hasTopic]->(t:Topic {display_name: 'X'})
MATCH (w)-[:mentions]->(e:Entity)
RETURN e.label, count(w) as mention_count
ORDER BY mention_count DESC
LIMIT 20
```

---

## 13. GNN Readiness Checklist

For RuVector Tier 2 integration:

| Requirement | Status | Action |
|-------------|--------|--------|
| Embeddings on Work nodes | ✅ Done | bge-m3 1024d |
| Embeddings on Entity nodes | ✅ Done | EntitySchema has embedding Vector field; ExtractionUseCase.with_embedder() |
| Embeddings on Section nodes | ✅ Done | SectionSchema has embedding Vector field |
| Edge weights | ✅ Done | set_edge_property_float in DirectGraphStore; MENTIONS weight=1.0 |
| Typed adjacency export | ❌ Missing | Export CSR matrices per edge type |
| heterogeneous node types | ✅ Done | 8+ node types |
| retrieval_eligible filter | ✅ Done | D134 on ALL nodes |
| Community detection | ❌ Future | RuVector GNN-rerank |
| PPR from any node | ❌ Future | RuVector solver |
| Agent assertions | ❌ Future | Layer 7 |

---

## 14. Migration Path

### Phase 1 (current): Rule-based extraction validation
- ✅ 100-paper corpus, P=0.775 R=0.999 F1=0.873
- ✅ Three-layer schema (L1-L3)
- ✅ Section title scanning, multi-word phrases

### Phase 2: Source provenance + multi-domain
- Add Layer 0 (Source nodes)
- Add TextbookProfile for GNN textbook chapters
- Add HTML parser adapter
- Ingest GNN textbook (4 chapters) into graph

### Phase 3: Entity embeddings + edge weights
- Add `embedding` field to Entity, Section nodes
- Compute bge-m3 embeddings for all entity labels
- Add `weight` property to all edges
- Export typed adjacency matrices

### Phase 4: Hypergraph + temporal queries
- Add ConceptCluster, MethodFamily, BenchmarkSuite nodes
- Implement MEMBER_OF edges
- Add temporal query patterns
- Add summary generation queries

### Phase 5: GNN integration (RuVector Tier 2)
- PPR-based search from any node
- GNN community detection
- SIMILAR_TO edges (GNN-derived)
- Agent assertions (Layer 7)

---

## 15. Domain Profiles (config-driven)

Each source domain has a profile that determines:
- Which fields are required vs optional
- Which parser adapter to use
- Which extraction patterns apply
- Which entity types are relevant

```yaml
# data/domain_profiles/paper.yaml
domain: scientific_paper
source_type: pdf
parser: grobid
extraction: rule_based
required_fields: [title, abstract_text, arxiv_id]
entity_types: [Method, Dataset, Model, Metric, Task]
summary_fields: [title, abstract, key_entities, topics]

# data/domain_profiles/textbook.yaml
domain: textbook
source_type: html
parser: html_parser
extraction: rule_based
required_fields: [title, chapter_number]
entity_types: [Method, Dataset, Model, Metric, Task, Concept]
summary_fields: [title, chapter_summary, key_concepts, prerequisites]

# data/domain_profiles/lecture.yaml
domain: lecture_notes
source_type: pdf
parser: grobid
extraction: rule_based
required_fields: [title, course_code]
entity_types: [Method, Dataset, Model, Metric, Task, Concept]
summary_fields: [title, lecture_summary, key_points]
```

---

## 16. Summary

This ontology design extends the current three-layer schema (L1-L3) to seven layers (L0-L7):

| Layer | Status | Key addition |
|-------|--------|-------------|
| **L0: Source** | NEW | Multi-source federation |
| **L1: Metadata** | Enhanced | Domain profiles, Source link |
| **L2: Structure** | Enhanced | Chapter, Paragraph, CodeBlock |
| **L3: Content** | Enhanced | Entity embeddings, domain tags |
| **L4: Relations** | Enhanced | Edge weights, typed adjacency |
| **L5: Temporal** | NEW | Snapshots, temporal queries |
| **L6: Hypergraph** | NEW | ConceptCluster, MethodFamily |
| **L7: Agent** | FUTURE | RuVector GNN, SIMILAR_TO |

Each layer is **independently deployable** — L0-L3 work now, L4-L5 are Phase 2-3, L6-L7 are Phase 4-5.

The design supports the user's requirements:
- ✅ Multi-source (L0 + L1 profiles)
- ✅ Multi-domain (L1 domain_profile)
- ✅ Subgraphs (L4 + queries)
- ✅ Hypergraph (L6)
- ✅ Temporality (L5)
- ✅ Summary (query patterns)
- ✅ GNN readiness (L3 embeddings + L4 weights + L7 agent)
