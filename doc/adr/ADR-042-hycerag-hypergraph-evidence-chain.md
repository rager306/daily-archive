# ADR-042: HyCE-RAG-Inspired Hypergraph Evidence Chain Model

**Status:** Proposed
**Date:** 2026-07-29
**Deciders:** collaborative
**Related:** ADR-037 (RuVector agent brain), ADR-040 (Samyama sole store), ONTOLOGY-DESIGN (Layer 6 Hypergraph)

## Context

ONTOLOGY-DESIGN Layer 6 defines ConceptCluster nodes for grouping entities
into higher-level concepts. However, the design lacks:

1. **Hyperedge semantics** — how multiple entities + evidence context form
   a single semantic unit (not just a label on a cluster)
2. **Incidence relation** — the entity↔hyperedge membership structure needed
   for confidence propagation
3. **Evidence chain construction** — post-retrieval reasoning that connects
   scattered evidence into explainable paths

Paper **HyCE-RAG** (arXiv:2607.22597, June 2026) provides a concrete
blueprint for addressing all three gaps using hypergraph-structured
knowledge representation with confidence-aware evidence chain construction.

## Decision

Adopt HyCE-RAG's **two-phase architecture** as the blueprint for daily-archive
Layer 6 (Hypergraph) and Layer 7 (Agent Memory):

### Phase A: Offline hypergraph construction (ADR-040 compliant)

- Extract entities, relations, and **hyperedges** (connecting multiple
  entities + context) from ingested papers
- Store in Samyama Graph: Entity nodes + ConceptCluster nodes + MEMBER_OF edges
- MEMBER_OF edges carry `weight` (extraction confidence) and `context` (evidence text)
- Entity embeddings enable semantic similarity (bge-m3, already wired)

### Phase B: Online evidence chain construction (future, RuVector Tier 2)

- **Entry-entity extraction**: query → entity matching via embedding similarity
- **Query-aware subhypergraph**: expand from entry entities via MEMBER_OF edges
- **Confidence propagation**: PPR-like algorithm over incidence structure
  (RuVector solver, with restart mechanism)
- **Evidence assembly**: confidence-guided path construction considering:
  - Semantic relevance (cosine similarity)
  - Entity connectivity (graph structure)
  - Evidence coverage (entity overlap)
  - Extraction confidence (edge weight)
  - Propagated confidence (PPR score)
- **Structured context**: merged evidence paths → LLM input (not flat chunks)

### Schema additions (Layer 6)

| Element | Type | Description |
|---------|------|-------------|
| ConceptCluster | Node | Hyperedge node (already implemented) |
| MEMBER_OF | Edge | Entity → ConceptCluster, carries weight + context |
| SUBSUMES | Edge | ConceptCluster → ConceptCluster (hierarchy) |
| EvidenceChain | Node | Result of Phase B online reasoning (future) |
| SUPPORTS | Edge | EvidenceChain → Entity (evidence link) |

### GNN readiness alignment

| HyCE-RAG concept | daily-archive status |
|------------------|---------------------|
| Hyperedge | ✅ ConceptCluster schema |
| Entity embeddings | ✅ bge-m3 1024d, wired |
| Incidence structure | ⏳ MEMBER_OF edge type (next) |
| Confidence propagation | ⏳ RuVector PPR (Phase 5) |
| Evidence chains | ⏳ Phase 5-6 |

## Consequences

**Positive:**
- Provides concrete blueprint for hypergraph evidence reasoning
- Aligns with RuVector Tier 2 (PPR solver already vendored)
- Enables explainable multi-hop QA over scientific literature
- ConceptCluster nodes already implemented — low incremental cost

**Negative:**
- MEMBER_OF edges with context property increase storage
- Confidence propagation requires RuVector integration (not yet wired)
- LLM-based hyperedge extraction may introduce noise (GSD: "statistical-first")

**Compliance:**
- ADR-040: Samyama sole store — hyperedges are nodes in Samyama ✅
- D127: import_eligible=false — agent assertions stay quarantined ✅
- D134: retrieval_eligible on ALL nodes ✅
- GSD memory: "statistical-first before every LLM" — offline extraction
  uses rule-based extractor first; LLM hyperedge extraction deferred ✅
