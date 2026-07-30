# ADR-042: Query-Local Evidence Activation Over Reified Evidence Bundles

**Status:** Proposed (revised — supersedes initial HyCE-RAG hypergraph blueprint)
**Date:** 2026-07-29 (revised)
**Deciders:** collaborative
**Related:** ADR-037 (RuVector agent brain), ADR-040 (Samyama sole store), ADR-038 (ExperimentSetup n-ary)

## Context

Initial draft of ADR-042 adopted HyCE-RAG (arXiv:2607.22597) as a blueprint
for hypergraph evidence chains. Deep review revealed semantic confusions:

1. **ConceptCluster ≠ EvidenceHyperedge** — ConceptCluster is a derived
   community object (co-occurrence cluster, method family). It does not
   represent a source-grounded evidence unit, claim, or reasoning step.
2. **MEMBER_OF conflates community membership with evidence participation** —
   entities co-occurring in N papers do not form a self-contained fact.
3. **SUPPORTS targets Entity** — entities are not truth-bearing propositions.
4. **Query-local evidence chains should not be canonical knowledge nodes** —
   they are ephemeral retrieval results (RVF Tier 3 on save).

## Revised decision

### Three distinct object types

```
ConceptCluster      — derived semantic community (existing, keep as-is)
  Entity → MEMBER_OF_CLUSTER → ConceptCluster
  Not evidence, not claim, not reasoning step.

EvidenceBundle      — source-grounded n-ary evidence unit (NEW)
  Entity → PARTICIPATES_IN {role} → EvidenceBundle
  Subtypes: ExperimentSetup, ResultBundle, CitationContext, ClaimBundle
  Fields: vid, bundle_type, normalized_text, source_span_id,
          document_id, section_id, extraction_confidence,
          verification_status, valid_from, valid_to, retrieval_eligible

Claim               — proposition-bearing node (NEW, future)
  EvidenceBundle → SUPPORTS → Claim
  EvidenceBundle → CONTRADICTS → Claim
  EvidenceBundle → QUALIFIES → Claim
  Fields: vid, text, claim_type, modality, scope,
          valid_time, source_span_id
```

### Query-local evidence activation (Phase 5+)

```
Query → candidate retrieval (BM25, dense, entity lookup)
      → query-local incidence graph (Entity ↔ EvidenceBundle only)
      → structural activation (PPR-like, non-uniform restart)
      → evidence subgraph assembly (connected, covers query anchors)
      → claim-level verification (supports/contradicts/qualifies)
      → structured context → LLM
```

Evidence chains are **ephemeral** — not stored as canonical knowledge.
If persisted: RVF Tier 3 (agent experience), `retrieval_eligible=false`.

### What stays unchanged

- ConceptCluster stays as derived community detection output
- detect_clusters() stays as offline community analysis
- PPR port trait stays, but needs weighted restart + diagnostics
- Samyama as sole store — EvidenceBundle is a node in Samyama
- Fail-closed import (D127)
- Statistical-first extraction (no LLM hyperedge extraction yet)

### PPR port improvement needed

Current:
```rust
personalized_pagerank(seed_nodes: &[u64], alpha: f64, max_iterations: usize)
```

Needed:
```text
restart_distribution: [(NodeId, weight)]  // non-uniform
restart_probability: f64                  // separate from max_iterations
tolerance: f64                            // convergence threshold
allowed_edge_types: &[&str]              // query-local scope
→ scores + iterations + residual + converged
```

### Incidence traversal must be bidirectional

```
Entity → PARTICIPATES_IN → EvidenceBundle
EvidenceBundle → HAS_PARTICIPANT → Entity
```

PPR must traverse both directions. Either store both edges or provide
undirected incidence view in adapter.

## Migration path

1. **P0 (now):** Rename MEMBER_OF → MEMBER_OF_CLUSTER in relation.rs.
   Add PARTICIPATES_IN edge type. Fix SUPPORTS → Claim target.
   Update cluster.rs docstrings (no "hyperedge" language).
2. **P1:** Add EvidenceBundle + Claim domain types.
   Generalize ExperimentSetup as EvidenceBundle subtype.
3. **P2:** Wire EvidenceBundle creation into extraction pipeline
   (when entities co-occur in same section with source span).
4. **P3:** Implement query-local incidence PPR (feature flag).
5. **P4:** Claim-level verifier (supports/contradicts/qualifies).

## What we do NOT do

- Do NOT run PPR over ConceptCluster/MEMBER_OF edges (wrong semantics).
- Do NOT store query-generated EvidenceDAG as canonical knowledge.
- Do NOT mix activation_score with extraction_confidence with
  source_reliability with verification_status. Four separate fields.
- Do NOT call top-15 bundles "evidence chains" — they are candidate
  bundles. Chain assembly is a separate algorithmic step.

## Consequences

**Positive:**
- Clean separation: community detection vs evidence retrieval
- EvidenceBundle is source-grounded (has source_span_id)
- Claim is truth-bearing (can be supported/contradicted)
- Query-local PPR won't be polluted by hubness/centrality

**Negative:**
- More node types (14 total: +EvidenceBundle, +Claim)
- PARTICATES_IN edges increase graph density
- Requires evidence extraction pipeline (future work)
