# FalkorDB Schema and Operators Design (M101 S04)

## Overview

Designs FalkorDB graph schema implementing ADR-028 typed knowledge schema (27 relations, 5 modules), vector indexes for BGE-M3 embeddings, 5 graph layers, and 6 graph operators adapted from Agents-K1. Includes migration plan from NetworkX/LadybugDB.

## Node Labels

| Label | Module | Key properties | Index |
|---|---|---|---|
| `Source` | A | `source_id`, `source_type`, `title`, `content_hash`, `fetched_at` | `source_id` (unique) |
| `Author` | A | `author_id`, `name`, `orcid` | `author_id` (unique) |
| `Venue` | A | `venue_id`, `name`, `type` | `venue_id` (unique) |
| `Resource` | A | `resource_id`, `kind`, `uri`, `version` | `resource_id` (unique) |
| `Entity` | B | `entity_id`, `entity_type`, `canonical_name`, `confidence` | `entity_id` (unique), `entity_type`, `canonical_name` |
| `Abstract` | C | `abstract_id`, `abstract_type`, `statement`, `evidence_span` | `abstract_id` (unique), `abstract_type` |
| `Citation` | D | `citation_id`, `cite_type`, `relation`, `quote`, `evidence_section` | `citation_id` (unique) |
| `Evidence` | — | `evidence_id`, `source_id`, `chunk_id`, `span_hash` | `evidence_id` (unique) |
| `KnowledgeCard` | C | `card_id`, `source_id`, `summary`, `methodology`, `key_findings` | `card_id` (unique) |

### Vector Index

```cypher
// FalkorDB supports vector similarity via FLAT or HNSW index
// BGE-M3 embeddings: 1024 dimensions, float32

CREATE VECTOR INDEX entity_embedding FOR (e:Entity) ON (e.embedding)
OPTIONS { dimension: 1024, similarityFunction: 'cosine' }

CREATE VECTOR INDEX card_embedding FOR (c:KnowledgeCard) ON (c.embedding)
OPTIONS { dimension: 1024, similarityFunction: 'cosine' }
```

## Edge Types (27 Typed Relations from ADR-028)

### Group 1: Controlled (domain-neutral)

```cypher
(:Entity)-[:BUILDS_ON     {confidence, extraction_id}]->(:Entity)
(:Entity)-[:USES_COMPONENT {confidence, extraction_id}]->(:Entity)
(:Entity)-[:ALTERNATIVE_TO {confidence, extraction_id}]->(:Entity)
(:Entity)-[:SOLVES         {confidence, extraction_id}]->(:Abstract)  // Method → Problem
(:Entity)-[:APPLIED_TO     {confidence, extraction_id}]->(:Entity)    // Method → Dataset
(:Entity)-[:TARGETS        {confidence, extraction_id}]->(:Entity)    // Method → Task
```

### Group 2: Causal

```cypher
(:Entity)-[:CAUSES          {confidence, extraction_id}]->(:Abstract)  // Factor → Finding
(:Entity)-[:ENABLES         {confidence, extraction_id}]->(:Entity)
(:Entity)-[:INHIBITS        {confidence, extraction_id}]->(:Abstract)
(:Entity)-[:MODULATES       {confidence, extraction_id}]->(:Entity)
(:Entity)-[:CORRELATED_WITH {confidence, extraction_id}]->(:Entity)
```

### Group 3: Composition

```cypher
(:Entity)-[:USES_TECHNIQUE {confidence, extraction_id}]->(:Entity)
(:Entity)-[:CONSISTS_OF    {confidence, extraction_id}]->(:Entity)
(:Entity)-[:IMPLEMENTS     {confidence, extraction_id}]->(:Entity)
(:Entity)-[:COMBINES       {confidence, extraction_id}]->(:Entity)
(:Entity)-[:REQUIRES       {confidence, extraction_id}]->(:Entity|:Resource)
```

### Group 4: Comparison

```cypher
(:Entity)-[:DERIVED_FROM      {confidence, extraction_id}]->(:Entity)
(:Entity)-[:DIFFERS_FROM      {confidence, extraction_id}]->(:Entity)
(:Entity)-[:HAS_LIMITATION    {confidence, extraction_id}]->(:Abstract)
(:Entity)-[:ADDRESSES_PROBLEM {confidence, extraction_id}]->(:Abstract)
(:Entity|:Abstract)-[:MOTIVATED_BY {confidence, extraction_id}]->(:Abstract)
(:Entity)-[:HAS_PROPERTY      {confidence, extraction_id}]->(:Entity)
(:Entity)-[:SUBSET_OF         {confidence, extraction_id}]->(:Entity)
```

### Group 5: Citation

```cypher
(:Source)-[:CITES    {citation_id, cite_type}]->(:Source)
(:Source)-[:SUPPORTS {citation_id, quote}]->(:Abstract|:Entity)
(:Source)-[:CONTRASTS {citation_id, quote}]->(:Entity|:Abstract)
(:Source)-[:EXTENDS   {citation_id, quote}]->(:Entity)
```

### Structural Edges (non-typed)

```cypher
(:Source)-[:HAS_AUTHOR   {order}]->(:Author)
(:Source)-[:PUBLISHED_IN]->(:Venue)
(:Source)-[:HAS_RESOURCE]->(:Resource)
(:Source)-[:HAS_ENTITY  ]->(:Entity)
(:Source)-[:HAS_ABSTRACT]->(:Abstract)
(:Source)-[:HAS_CARD    ]->(:KnowledgeCard)
(:Entity|:Abstract)-[:EVIDENCED_BY]->(:Evidence)
(:Entity)-[:HAS_EMBEDDING {dimension: 1024}]->(:Entity)  // self-referential for vector
```

## Graph Layers

### Layer 0: Source Registry
```cypher
// Universal source registration
(:Source {source_type: 'paper|textbook|code|dataset|tech_doc'})
(:Source)-[:HAS_AUTHOR]->(:Author)
(:Source)-[:PUBLISHED_IN]->(:Venue)
(:Source)-[:HAS_RESOURCE]->(:Resource)
```

### Layer 1: Entity Graph (Module B)
```cypher
// Typed entities extracted from sources
(:Source)-[:HAS_ENTITY]->(:Entity {entity_type: 'Method|Dataset|Metric|Task|...'})
(:Entity)-[:BUILDS_ON]->(:Entity)
(:Entity)-[:USES_COMPONENT]->(:Entity)
(:Entity)-[:ALTERNATIVE_TO]->(:Entity)
```

### Layer 2: Abstract Graph (Module C)
```cypher
// Implicit/abstracted concepts
(:Source)-[:HAS_ABSTRACT]->(:Abstract {abstract_type: 'Problem|Motivation|Gap|...'})
(:Entity)-[:SOLVES]->(:Abstract)
(:Entity)-[:HAS_LIMITATION]->(:Abstract)
(:Source)-[:HAS_CARD]->(:KnowledgeCard)
```

### Layer 3: Relationship Graph (Module E)
```cypher
// All 27 typed relation edges
// Causal, Composition, Comparison edges between entities and abstracts
(:Entity)-[:ENABLES]->(:Entity)
(:Entity)-[:INHIBITS]->(:Abstract)
(:Entity)-[:DERIVED_FROM]->(:Entity)
```

### Layer 4: Evidence Graph
```cypher
// Provenance and traceability
(:Entity)-[:EVIDENCED_BY]->(:Evidence {source_id, chunk_id, span_hash})
(:Abstract)-[:EVIDENCED_BY]->(:Evidence)
// Each Evidence links back to SemanticChunk + EvidencePath
```

## Graph Operators (O1-O6)

### O1: Seed Resolution

**Purpose**: Resolve mention strings to canonical entity nodes.

```cypher
// Input: list of mention strings
// Output: canonical entity sets

MATCH (e:Entity)
WHERE e.canonical_name IN $mention_strings
RETURN e.entity_id, e.entity_type, e.canonical_name

// Fuzzy match via embedding similarity
CALL db.idx.vector.queryNodes('Entity', 10, $query_embedding)
YIELD node, score
WHERE score > 0.85
RETURN node.entity_id, node.canonical_name, score
```

**Current status**: ✅ Partially implemented in `identity/canonicalization.py` (hash-based). Needs vector-based fuzzy matching in FalkorDB.

### O2: Citation Lineage Reconstruction

**Purpose**: Forward/backward citation traversal with shortest-path.

```cypher
// Forward citations (who cites this paper)
MATCH (s:Source)-[:CITES]->(target:Source {source_id: $source_id})
RETURN s

// Backward citations (who this paper cites)
MATCH (source:Source {source_id: $source_id})-[:CITES]->(target:Source)
RETURN target

// Shortest path between two papers
MATCH p = shortestPath((s1:Source {source_id: $from})-[:CITES*]->(s2:Source {source_id: $to}))
RETURN p
```

**Current status**: ✅ BFS 2-hop implemented (M064). Needs FalkorDB Cypher version.

### O3: Comparative Baseline Retrieval

**Purpose**: Find methods evaluated on a given dataset/metric.

```cypher
// Methods applied to a specific dataset
MATCH (m:Entity {entity_type: 'Method'})-[:APPLIED_TO]->(d:Entity {entity_type: 'Dataset', canonical_name: $dataset_name})
RETURN m.canonical_name, m.confidence

// Methods targeting a specific task
MATCH (m:Entity {entity_type: 'Method'})-[:TARGETS]->(t:Entity {entity_type: 'Task', canonical_name: $task_name})
RETURN m.canonical_name
```

**Current status**: ⬜ Needs typed relation index in FalkorDB.

### O4: Multimodal Anchor Retrieval

**Purpose**: Retrieve figures, tables, equations by semantic similarity.

```cypher
// Find figures related to a query
CALL db.idx.vector.queryNodes('Entity', 10, $query_embedding)
YIELD node, score
WHERE node.entity_type IN ['Figure', 'Table', 'Equation']
RETURN node, score

// Tables with specific column structure
MATCH (t:Entity {entity_type: 'Table'})
WHERE t.column_names CONTAINS $column_name
RETURN t
```

**Current status**: ⚠️ `source_assets/registry.py` exists but no vector retrieval. Needs FalkorDB vector index.

### O5: Gap Detection

**Purpose**: Find orphan methods, singleton datasets, sparse areas.

```cypher
// Methods without any TASK relation (orphan)
MATCH (m:Entity {entity_type: 'Method'})
WHERE NOT (m)-[:TARGETS]->()
RETURN m.canonical_name AS orphan_method

// Datasets used by only one method (singleton)
MATCH (d:Entity {entity_type: 'Dataset'})
WITH d, COUNT { (m:Entity)-[:APPLIED_TO]->(d) } AS usage_count
WHERE usage_count <= 1
RETURN d.canonical_name, usage_count

// Sparse Method-Task cells
MATCH (m:Entity {entity_type: 'Method'}), (t:Entity {entity_type: 'Task'})
WHERE NOT (m)-[:TARGETS]->(t)
AND EXISTS { (m)-[:APPLIED_TO]->(:Entity)-[:SUBSET_OF]->(t) }
RETURN m.canonical_name, t.canonical_name AS potential_gap
```

**Current status**: ⬜ Needs typed relation graph in FalkorDB.

### O6: Idea Grounding / Novelty Judging

**Purpose**: Check if a proposed idea overlaps with existing work.

```cypher
// Find methods with similar problem formulation
MATCH (m:Entity {entity_type: 'Method'})-[:SOLVES]->(p:Abstract {abstract_type: 'Problem'})
WHERE p.statement CONTAINS $problem_keywords
RETURN m.canonical_name, p.statement

// Check algorithmic mechanism overlap
MATCH (m:Entity {entity_type: 'Method'})-[:USES_TECHNIQUE]->(t:Entity)
WHERE t.canonical_name IN $proposed_techniques
RETURN m.canonical_name, collect(t.canonical_name) AS overlapping_techniques
```

**Current status**: ⚠️ RLM `graph_traversal.py` prototype. Needs typed relation queries.

## Vector Index Strategy

| Node type | Vector source | Dimension | Index type | Query use |
|---|---|---|---|---|
| `Entity` | BGE-M3(canonical_name + context) | 1024 | HNSW | O1 seed resolution, O4 multimodal |
| `KnowledgeCard` | BGE-M3(summary + findings) | 1024 | HNSW | Semantic search, O6 novelty |
| `Abstract` | BGE-M3(statement) | 1024 | HNSW | O5 gap detection, O6 grounding |

**Embedding pipeline**: fd service (ADR-019) → BGE-M3 → 1024d float32 → FalkorDB vector index.

## Migration Plan: NetworkX/LadybugDB → FalkorDB

### Current State

| Component | Uses | Coupling |
|---|---|---|
| `graph/ladybug_client.py` | LadybugDB direct | Write/read graph operations |
| `graph/readiness/persistence.py` | LadybugDB | Graph-readiness package persistence |
| `retrieval/hybrid.py` | LadybugDB (read-only) | Vector + graph hybrid retrieval |
| NetworkX (ADR-016) | In-process | Intermediate representation |

### Migration Phases

#### Phase 3a: Add FalkorDB client (alongside LadybugDB)

```python
# research_graph/graph/falkordb_client.py (NEW)
class FalkorDBClient:
    """FalkorDB client for typed graph operations."""
    
    def __init__(self, host: str = "localhost", port: int = 6379):
        self._graph_name = "daily_archive"
        # FalkorDB uses Redis protocol
    
    def create_schema(self) -> None:
        """Create indexes and constraints."""
        ...
    
    def write_entity(self, entity: TypedEntity) -> str:
        """Write typed entity node."""
        ...
    
    def write_relation(self, relation: TypedRelation) -> str:
        """Write typed relation edge."""
        ...
```

**Rule**: FalkorDB client is ADDITIVE. LadybugDB continues working. Both receive writes during transition.

#### Phase 3b: Migrate read paths

- `retrieval/hybrid.py`: switch graph expansion from LadybugDB to FalkorDB
- `graph/readiness/retrieval_validation.py`: update to query FalkorDB
- NetworkX intermediate remains unchanged (ADR-016)

#### Phase 3c: Migrate write paths

- `graph/readiness/persistence.py`: switch from LadybugDB to FalkorDB
- Extraction pipeline output → FalkorDB write (through review gate)
- LadybugDB write path deprecated

#### Phase 3d: Deprecate LadybugDB

- Remove `graph/ladybug_client.py` (archive)
- Remove LadybugDB dependency from pyproject.toml
- FalkorDB is sole production GraphDB (ADR-022)

### Acceptance Criteria for Migration

1. FalkorDB schema created with all 27 typed edges
2. Vector indexes created and tested
3. Entity/relation write/read roundtrip tested
4. All graph operators O1-O6 return correct results
5. Hybrid retrieval (vector + graph) works via FalkorDB
6. LadybugDB code paths removed or archived
7. No test regressions

### Risk Mitigation

| Risk | Mitigation |
|---|---|
| FalkorDB vector index performance | Benchmark vs LadybugDB before full switch |
| Cypher learning curve | Start with simple queries, build operators incrementally |
| Data loss during migration | Dual-write during Phase 3a-3b; verify both stores match |
| LadybugDB coupling depth | `graph/readiness/persistence.py` may need refactoring |

## FalkorDB Deployment

```yaml
# docker-compose.yml (future)
falkordb:
  image: falkordb/falkordb:latest
  ports:
    - "6379:6379"
  volumes:
    - falkordb_data:/data
  environment:
    - REDIS_ARGS=--save 60 1
```

**Local-first**: FalkorDB runs locally via Docker. No cloud dependency.

## Quant-Mind Pattern Integration

| Pattern | FalkorDB implementation |
|---|---|
| TreeKnowledge | Source → KnowledgeCard hierarchy via HAS_CARD edges |
| GraphKnowledge | Typed edges (27 types) = quant-mind's placeholder realized |
| Knowledge store interface | FalkorDB client implements KnowledgeSubstratePort |
| Typed resolver | O1 (Seed Resolution) with vector + exact match |
