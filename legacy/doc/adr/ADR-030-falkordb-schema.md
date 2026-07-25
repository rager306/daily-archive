# ADR-030: FalkorDB Graph Schema and Operators

**Status:** Accepted (binding)  
**Date:** 2026-06-18  
**Deciders:** collaborative  
**Milestone:** M101-f5jip0 S04  
**Scope:** graphdb / schema / operators / migration  
**Binding Level:** binding  
**Revisable:** yes, with FalkorDB implementation evidence

## 0. One-line Decision

> daily-archive will implement a FalkorDB graph schema with 9 node labels, 27 typed edge types (ADR-028), 2 vector indexes (Entity + KnowledgeCard, BGE-M3 1024d), 5 graph layers, and 6 graph operators (O1-O6) adapted from Agents-K1. Migration from NetworkX/LadybugDB follows a 4-phase additive approach.

## 1. Context

ADR-022 binds FalkorDB as production GraphDB. ADR-028 defines typed schema (27 relations). Current code uses LadybugDB (superseded by ADR-022) coupled in `graph/ladybug_client.py` and `graph/readiness/persistence.py`. NetworkX (ADR-016) remains intermediate.

## 2. Decision

### 2.1 Node Labels (9)

Source, Author, Venue, Resource, Entity, Abstract, Citation, Evidence, KnowledgeCard.

### 2.2 Typed Edges (27 + structural)

All 27 relation types from ADR-028 as FalkorDB edge types. Plus structural edges (HAS_AUTHOR, HAS_ENTITY, EVIDENCED_BY, etc.).

### 2.3 Vector Indexes

- `Entity` nodes: BGE-M3(canonical_name + context), 1024d, HNSW
- `KnowledgeCard` nodes: BGE-M3(summary + findings), 1024d, HNSW

### 2.4 Graph Operators (O1-O6)

| Operator | Cypher pattern | Status |
|---|---|---|
| O1: Seed Resolution | Exact match + vector similarity > 0.85 | Extend identity/ |
| O2: Citation Lineage | shortestPath via CITES edges | Port from BFS 2-hop |
| O3: Comparative Baseline | APPLIED_TO + TARGETS traversal | New |
| O4: Multimodal Anchor | Vector query on Figure/Table/Equation | New |
| O5: Gap Detection | Orphan/singleton/sparse detection | New |
| O6: Novelty Grounding | SOLVES + USES_TECHNIQUE overlap | Extend RLM |

### 2.5 Migration Plan (4 phases)

| Phase | Action | Risk |
|---|---|---|
| 3a | Add FalkorDB client alongside LadybugDB | Low: additive |
| 3b | Migrate read paths (hybrid retrieval) | Medium: query equivalence |
| 3c | Migrate write paths (persistence) | Medium: dual-write verification |
| 3d | Deprecate LadybugDB | Low: archive only |

**Rule**: Dual-write during transition. Both stores must match before LadybugDB removal.

## 3. Applies To

- Layer 4 (Graph) of 7-layer architecture (ADR-023)
- Typed schema storage (ADR-028)
- Extraction output persistence (ADR-029)
- Agent graph operators (ADR-026)

## 4. LLM Reading Notes

- **Binding**: FalkorDB schema with 27 typed edges and 2 vector indexes.
- **Migration**: 4-phase additive approach. LadybugDB not removed until acceptance tests pass.
- **NetworkX** (ADR-016) remains in-process intermediate.
- **Operators O1-O6**: adapted from Agents-K1, implemented as Cypher queries.
- **Not authorized**: graph writes without review gate, production imports.
