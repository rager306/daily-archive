# Ontology Design v2 — Layers, Connectivity, Triplets, Subgraphs

**Status:** Proposed (companion to ADR-046/047/048/049)
**Date:** 2026-07-24
**Related:** ADR-042 (Claim/EvidenceBundle), ADR-043 (process plane),
ADR-044 (schema lifecycle), ADR-045 (validator), ADR-046 (bi-temporal),
ADR-047 (conflicts), ADR-048 (decisions), ADR-049 (pipeline DSL),
ONTOLOGY-DESIGN.md (v1), GRAPH-SCHEMA.md

This document specifies the **post-Semantica-cross-pollination** ontology:
what changes, what stays, how layers reorganize, how connectivity improves,
and what triplets/subgraphs the new design enables. v1 (ONTOLOGY-DESIGN.md)
remains authoritative for everything not touched here.

---

## 1. Layer reorganization (v1 → v2)

v1 defined 7 layers (L0–L6 + L7 future). v2 promotes two existing concerns
into first-class layers and adds two new ones.

| Layer | v1 name           | v2 name                          | Change                                       |
|-------|-------------------|----------------------------------|----------------------------------------------|
| L0    | Source Provenance | Source Provenance                | unchanged                                    |
| L1    | Metadata          | Metadata                         | unchanged                                    |
| L2    | Structure         | Structure                        | unchanged                                    |
| L3    | Content           | Content                          | unchanged                                    |
| L4    | Relations         | Relations                        | unchanged                                    |
| L5    | Temporal          | **Temporal (BiTemporal)**        | ADR-046 — splits valid/transaction time      |
| L6    | Evidence/Community| Evidence/Community               | unchanged                                    |
| L7    | (future)          | **Conflict Resolution (NEW)**    | ADR-047 — first-class conflict objects       |
| L8    | —                 | **Decision Intelligence (NEW)**  | ADR-048 — first-class decision records       |
| L9    | —                 | **Process (re-scoped from ADR-043)** | now depends on L5/L6/L7/L8              |
| L10   | (RVF future)      | Agent Memory (RVF)               | unchanged (future)                           |

### Why two new layers

v1 conflated **"things extracted from papers"** (L6 Evidence/Community)
with **"things our system does to the graph"** (healing, conflict
resolution, decisions). v2 separates them: L6 is source-derived; L7/L8 are
system-derived. This separation matters because:

- Source-derived facts are immutable (the paper said X; we may extract
  more, but we cannot un-say X). They live under D134 retrieval_eligible.
- System-derived facts are mutable (we may resolve a conflict, supersede
  a decision, correct an extraction). They need bi-temporal fields
  (ADR-046) and decision provenance (ADR-048).

### Layer dependency invariant (enforced by validator)

```
L0 Source        ← depends on: nothing
L1 Metadata      ← depends on: L0
L2 Structure     ← depends on: L1
L3 Content       ← depends on: L2
L4 Relations     ← depends on: L3
L5 Temporal      ← cross-cutting (attaches to L1–L4, L6–L9)
L6 Evidence      ← depends on: L3
L7 Conflict      ← depends on: L6
L8 Decision      ← depends on: L6, L7
L9 Process       ← depends on: L6, L7, L8
L10 AgentMemory  ← depends on: L8, L9
```

Cross-layer edges are allowed but always point from higher to lower
(e.g. `Decision RESOLVES Conflict` is L8 → L7, never the reverse).
Validator (ADR-045 Wave G extension) will enforce this directional rule.

---

## 2. Schema inventory (v2)

| #  | Label              | Layer | v1 status    | v2 status           | Delta                                     |
|----|--------------------|-------|--------------|---------------------|-------------------------------------------|
| 1  | Source             | L0    | materialized | materialized        | +bi-temporal fields optional              |
| 2  | Paper              | L1    | materialized | materialized        | +valid_from already; +recorded_at         |
| 3  | Author             | L1    | materialized | materialized        | +valid_from/valid_to for affiliation      |
| 4  | Institution        | L1    | materialized | materialized        | +openalex_pending temporal                |
| 5  | Topic              | L1    | materialized | materialized        | unchanged                                 |
| 6  | Category           | L1    | materialized | materialized        | unchanged                                 |
| 7  | Concept (legacy)   | L1    | deprecated   | deprecated          | unchanged                                 |
| 8  | Section            | L2    | materialized | materialized        | unchanged                                 |
| 9  | Reference          | L2    | materialized | materialized        | +valid_from already                       |
| 10 | Citation           | L2    | materialized | materialized        | unchanged                                 |
| 11 | Entity             | L3    | materialized | materialized        | unchanged                                 |
| 12 | ConceptCluster     | L6    | materialized | materialized        | unchanged                                 |
| 13 | EvidenceBundle     | L6    | materialized | materialized        | +recorded_at (ADR-046)                    |
| 14 | Claim              | L6    | materialized | materialized        | +valid_to/recorded_at/superseded_at       |
| 15 | SchedulerTask      | ops   | materialized | materialized        | unchanged                                 |
| 16 | ResearchProblem    | L9    | materialized | materialized        | +bi-temporal                              |
| 17 | MetricObservation  | L9    | materialized | materialized        | +bi-temporal; Conflict participant (metric) |
| 18 | ResearchEnvironment| L9    | declared     | declared            | future materialization                    |
| 19 | BaselineSnapshot   | L9    | declared     | declared            | future                                    |
| 20 | ResearchIdea       | L9    | declared     | declared            | future                                    |
| 21 | Hypothesis         | L9    | declared     | declared            | future                                    |
| 22 | Intervention       | L9    | declared     | declared            | future                                    |
| 23 | InterventionBundle | L9    | declared     | declared            | future                                    |
| 24 | ImplementationAttempt | L9  | declared     | declared            | future                                    |
| 25 | ArtifactVersion    | L9    | declared     | declared            | future                                    |
| 26 | ExperimentRun      | L9    | declared     | declared            | future; bi-temporal                        |
| 27 | MetricDefinition   | L9    | declared     | declared            | future                                    |
| 28 | ResultComparison   | L9    | declared     | declared            | future; bi-temporal                        |
| 29 | FailureEvent       | L9    | declared     | declared            | future                                    |
| 30 | **Conflict**       | **L7**| —            | **new (ADR-047)**   | materialize in ADR-047 Phase 1            |
| 31 | **Decision**       | **L8**| —            | **new (ADR-048)**   | materialize in ADR-048 Phase 1            |

**Totals**: 31 schemas (was 29), 17 materialized (unchanged for now),
14 declared-but-future (was 13).

---

## 3. Edge inventory (v2)

| Edge constant         | v1 | v2 | Source → Target(s) (polymorphic)                | Layer |
|-----------------------|----|----|-------------------------------------------------|-------|
| FROM_SOURCE           | ✓  | ✓  | Paper → Source                                  | L0→L1 |
| AUTHORED_BY           | ✓  | ✓  | Author → Paper                                  | L1    |
| AFFILIATED_WITH       | ✓  | ✓  | Author → Institution                            | L1    |
| HAS_TOPIC             | ✓  | ✓  | Paper → Topic                                   | L1    |
| IN_CATEGORY           | ✓  | ✓  | Paper → Category                                | L1    |
| HAS_PART              | ✓  | ✓  | Paper → Section · Reference                     | L2    |
| FOUND_IN              | ✓  | ✓  | Entity → Section                                | L3    |
| MENTIONS              | ✓  | ✓  | Paper → Entity · ResearchProblem · MetricObservation | L1→L3/L9 |
| CITES                 | ✓  | ✓  | Paper → Citation                                | L2    |
| SUPERSEDES            | ✓  | ✓  | Entity → Entity                                 | L3/heal |
| MEMBER_OF_CLUSTER     | ✓  | ✓  | Entity → ConceptCluster                         | L6    |
| PARTICIPATES_IN       | ✓  | ✓  | Entity → EvidenceBundle                         | L3→L6 |
| SUPPORTS              | ✓  | ✓  | EvidenceBundle → Claim                          | L6    |
| REFUTES               | ✓  | ✓  | EvidenceBundle → Claim                          | L6    |
| **CONFLICTS_OVER**    | —  | **new** | Claim/Entity/Reference/MetricObservation → Conflict (hyperedge) | L6→L7 |
| **RESOLVED_BY**       | —  | **new** | Conflict → Source · Decision                    | L7→L0/L8 |
| **CAUSED**            | —  | **new** | Decision → Decision (transitive)                | L8    |
| **INFLUENCED**        | —  | **new** | Decision → Decision (non-transitive)            | L8    |
| **PRECEDENT_FOR**     | —  | **new** | Decision → Decision                             | L8    |
| **AUTHORITY_FOR**     | —  | **new** | Policy → Decision                               | L8 (future) |
| **TRIGGERED_BY**      | —  | **new** | Decision ← Conflict · HealingAction · ExtractionOverride | L8←L7/ops |

**Totals**: 21 edge constants (was 13). 8 new edges for L7 (Conflict) and L8 (Decision).

### Edge layer direction invariant

All new edges respect the layer dependency invariant (§1). Validator rule:
edge source_layer > target_layer is forbidden unless the edge is explicitly
cross-cutting (e.g. MENTIONS L1→L9 is allowed because Paper MENTIONS
process-plane nodes; RESOLVED_BY L7→L8 is allowed because Decisions resolve
Conflicts).

---

## 4. Connectivity analysis

### 4.1 Graph density (current v1 vs projected v2)

Assuming the existing 200-paper canary corpus:

| Metric                    | v1 estimate | v2 estimate | Delta                 |
|---------------------------|-------------|-------------|-----------------------|
| Nodes (publication)       | ~3500       | ~3500       | unchanged             |
| Nodes (process plane)     | ~400        | ~400        | unchanged             |
| Nodes (Conflict, new)     | 0           | ~150        | factual/metric/temporal conflicts |
| Nodes (Decision, new)     | 0           | ~500        | healing + conflict + override decisions |
| Edges (publication)       | ~12000      | ~12000      | unchanged             |
| Edges (process)           | ~800        | ~800        | unchanged             |
| Edges (conflict/decision) | 0           | ~2000       | CONFLICTS_OVER + RESOLVED_BY + causal |
| **Average degree**        | ~6.5        | ~7.5        | +1 per Decision/Conflict layer |

### 4.2 Hub analysis (projected top-10 by degree)

| Node                          | Expected degree | Why                                  |
|-------------------------------|-----------------|--------------------------------------|
| Paper (per paper)             | 20–50           | Section/Entity/Author edges          |
| Entity (canonical)            | 10–100          | MENTIONS + PARTICIPATES_IN + MEMBER_OF_CLUSTER |
| Claim                         | 5–20            | SUPPORTS + REFUTES + CONFLICTS_OVER  |
| Conflict                      | 3–10            | 2+ CONFLICTS_OVER + 1 RESOLVED_BY    |
| Decision (healing root)       | 10–50           | CAUSED chain                         |
| Source (arXiv)                | 200             | FROM_SOURCE for every paper          |

### 4.3 Connected components

v1: weakly connected components cluster by paper (one component per paper,
plus orphan Entity nodes). Cross-paper connectivity exists only through
shared Topic / Author / Category / ConceptCluster.

v2 adds:
- **Conflict components** — multi-paper components via CONFLICTS_OVER.
  Two papers claiming contradictory things about the same entity now share
  a component through the Conflict node.
- **Decision components** — healing/override decisions cluster around the
  entity they touched.
- **Causal chains** — Decisions linked by CAUSED/INFLUENCED form chains
  that may span many papers.

Expected: average component size grows ~15% as Conflicts/Decisions bridge
previously-disconnected paper components.

---

## 5. Triplet inventory (subject-predicate-object patterns)

### 5.1 v1 triplets (existing, 13 edge types → ~50 distinct triplet shapes)

```text
(Paper, FROM_SOURCE, Source)
(Paper, AUTHORED_BY, Author)   ← Author→Paper actually; listing s-o-o order
(Section, HAS_PART, Paper)     ← inverted for readability; actual Paper→Section
(Entity, MENTIONS, Paper)
(Entity, FOUND_IN, Section)
(Entity, PARTICIPATES_IN, EvidenceBundle)
(EvidenceBundle, SUPPORTS, Claim)
(EvidenceBundle, REFUTES, Claim)
(Entity, MEMBER_OF_CLUSTER, ConceptCluster)
(Entity, SUPERSEDES, Entity)
(Paper, CITES, Citation)
(Author, AFFILIATED_WITH, Institution)
(Paper, HAS_TOPIC, Topic)
(Paper, IN_CATEGORY, Category)
(Paper, MENTIONS, ResearchProblem)
(Paper, MENTIONS, MetricObservation)
(Section, HAS_PART, Paper) variant
(Reference, HAS_PART, Paper) variant
```

### 5.2 v2 new triplets (8 new edge types → ~25 new triplet shapes)

```text
(Claim, CONFLICTS_OVER, Conflict)
(Entity, CONFLICTS_OVER, Conflict)
(Reference, CONFLICTS_OVER, Conflict)
(MetricObservation, CONFLICTS_OVER, Conflict)
(Conflict, RESOLVED_BY, Source)
(Conflict, RESOLVED_BY, Decision)
(Decision, CAUSED, Decision)
(Decision, INFLUENCED, Decision)
(Decision, PRECEDENT_FOR, Decision)
(Policy, AUTHORITY_FOR, Decision)
(Decision, TRIGGERED_BY, Conflict)
(Decision, TRIGGERED_BY, HealingAction)
(Decision, TRIGGERED_BY, ExtractionOverride)
```

Plus hyperedge-style (multiple participants):
```text
{(Claim_A, Claim_B), CONFLICTS_OVER, Conflict_1}
{(MetricObs_0.92, MetricObs_0.87), CONFLICTS_OVER, Conflict_2}
```

### 5.3 Triplet-level invariants (validator rules)

- `CONFLICTS_OVER` source must be a fact-bearing node (Claim/Entity/
  Reference/MetricObservation). Validator rule `conflict-participant-kind`.
- `RESOLVED_BY` target must be Source or Decision. Rule `resolve-target-kind`.
- `CAUSED` cycle detection: no cycles in CAUSED graph (DAG invariant).
  Rule `causal-acyclic`.
- `PRECEDENT_FOR` is non-transitive (a precedent is a citation, not a chain).
- Every Conflict must have ≥2 `CONFLICTS_OVER` incoming edges. Rule
  `conflict-min-participants`.

---

## 6. Subgraph patterns (query recipes)

### 6.1 "Why did the system silence this entity?" (v2 new)

```cypher
MATCH (e:Entity {vid: $vid})<-[:TRIGGERED_BY]-(d:Decision {category:'healing_action'})
OPTIONAL MATCH (d)-[:CAUSED*0..5]->(root:Decision)
RETURN e, d, root
```

Replaces `git log` archaeology in `.gsd/` and healing.rs free-text `reason`.

### 6.2 "What did we know about method X on date Y?" (v2 new — bi-temporal)

```cypher
MATCH (e:Entity {label: $method})<-[:MENTIONS]-(p:Paper),
      (e)<-[:SUBJECT]-(c:Claim)
WHERE c.valid_from <= $date
  AND (c.valid_to = 'OPEN' OR c.valid_to > $date)
  AND c.recorded_at <= $date
  AND (c.superseded_at = 'OPEN' OR c.superseded_at > $date)
RETURN p, c
```

Valid time (paper publication + claim retraction) intersects with
transaction time (when our system learned it).

### 6.3 "Which papers disagree about method X's accuracy?" (v2 new — conflict)

```cypher
MATCH (m:MetricObservation)-[:CONFLICTS_OVER]->(c:Conflict {kind:'metric'})
MATCH (m)<-[:MENTIONS]-(p:Paper)
RETURN p.title, m.value, c.status
ORDER BY c.detected_at DESC
```

### 6.4 "Trace the causal chain of this retraction" (v2 new — decision)

```cypher
MATCH path = (d:Decision {category:'retraction_recorded'})-[:CAUSED*]->(leaf:Decision)
WHERE d.vid = $decision_vid
RETURN path
```

### 6.5 v1 subgraph patterns (preserved)

- Citation neighborhood (existing): 2-hop CITES traversal.
- Topic cluster (existing): Paper → HAS_TOPIC → Topic + sibling Papers.
- Method lineage (existing): ConceptCluster → MEMBER_OF_CLUSTER → Entity + Authors.
- Cross-source subgraph (existing): Paper → FROM_SOURCE → Source + all Papers from same Source.

---

## 7. Completeness analysis

### 7.1 What v2 covers that v1 did not

| Question                                              | v1 answer        | v2 answer                            |
|-------------------------------------------------------|------------------|--------------------------------------|
| "Why did the system heal/merge this entity?"          | `git log` + grep | Decision subgraph (ADR-048)          |
| "Did two papers disagree about this?"                 | silent overwrite | Conflict node (ADR-047)              |
| "What was known on date X?"                           | ❌ no             | bi-temporal query (ADR-046)          |
| "Was this fact retracted by the source?"              | ❌ no             | valid_to + Conflict (ADR-046/047)    |
| "What did the agent decide last run and why?"         | structured logs  | Decision category scan (ADR-048)     |
| "Which precedent did this re-run follow?"             | ❌ no             | PRECEDENT_FOR chain (ADR-048)        |
| "Which pipeline template produced this graph state?"  | ❌ no             | ExecutionResult + PipelineRun (ADR-049) |
| "Is the pipeline about to fail before it runs?"       | runtime error    | PipelineValidator pre-flight (ADR-049) |

### 7.2 What v2 still does not cover (explicit non-goals)

- **Policy rule engine (SHACL)** — Phase 2 of ADR-048. v2 records `policy_id`
  as a string but does not materialize Policy nodes or evaluate rules.
- **Polyglot storage (RDF + LPG simultaneously)** — considered from Semantica
  but rejected for daily-archive (Samyama schemaless is sufficient; ADR-040).
- **Node2Vec embeddings for graph similarity** — Phase 2 of GNN readiness;
  reasoning_embedding on Decision is placeholder.
- **Cross-agent decision sharing** — single-agent only in v2.
- **Multi-modal provenance (images, figures, tables)** — Reference and
  Section carry text provenance only; figure/table extraction is future.

### 7.3 Layer coverage scorecard

| Layer | Schemas | Materialized | Edges | Bi-temporal | Validator rules |
|-------|---------|--------------|-------|-------------|-----------------|
| L0 Source      | 1  | ✅ | 1  | optional | ✅              |
| L1 Metadata    | 6  | ✅ | 5  | partial | ✅              |
| L2 Structure   | 3  | ✅ | 2  | partial | ✅              |
| L3 Content     | 1  | ✅ | 3  | ❌      | ✅              |
| L4 Relations   | —  | n/a | — | n/a     | ✅ (relation types) |
| L5 Temporal    | —  | cross | — | ✅ (ADR-046) | ⚠️ (Phase 3) |
| L6 Evidence    | 3  | ✅ | 4  | partial | ✅              |
| L7 Conflict    | 1  | ⚠️ (Phase 1) | 2 | ✅ | ⚠️ (Phase 1)   |
| L8 Decision    | 1  | ⚠️ (Phase 1) | 5 | ✅ | ⚠️ (Phase 1)   |
| L9 Process     | 14 | ⚠️ (Wave 2) | — | partial | ✅              |
| L10 AgentMemory| —  | ❌ (future RVF) | — | ❌ | ❌              |

### 7.4 Connectivity completeness scorecard

| Connectivity type                   | v1 | v2 |
|-------------------------------------|----|----|
| Paper ↔ Author                      | ✅ | ✅ |
| Paper ↔ Entity (content)            | ✅ | ✅ |
| Paper ↔ Reference (citation)        | ✅ | ✅ |
| Entity ↔ EvidenceBundle ↔ Claim     | ✅ | ✅ |
| Entity ↔ ConceptCluster             | ✅ | ✅ |
| **Claim ↔ Conflict**                | ❌ | ✅ (L7) |
| **MetricObservation ↔ Conflict**    | ❌ | ✅ (L7) |
| **Conflict ↔ Decision (resolution)**| ❌ | ✅ (L7→L8) |
| **Decision ↔ Decision (causal)**    | ❌ | ✅ (L8) |
| **Decision ↔ HealingAction**        | ❌ | ✅ (L8←ops) |
| Paper ↔ Topic / Category            | ✅ | ✅ |
| Author ↔ Institution                | ✅ | ✅ |
| Cross-time (bi-temporal traversal)  | ❌ | ✅ (L5) |
| Cross-paper (through Conflict)      | ❌ | ✅ (L7) |
| Cross-run (through PipelineRun)     | ❌ | ⚠️ Phase 2 |

---

## 8. Migration path (v1 → v2)

### Phase 0 (this document)

ADRs 046–049 proposed. No code change. Ontology design agreed.

### Phase 1: Bi-temporal + Conflict + Decision foundations

1. ADR-046: add bi-temporal optional fields to Claim, EvidenceBundle,
   MetricObservation. No required migration yet.
2. ADR-047 Phase 1: materialize Conflict node + CONFLICTS_OVER/RESOLVED_BY
   edges. ConflictDetectionUseCase detects factual/metric conflicts.
3. ADR-048 Phase 1: materialize Decision node + causal edges.
   DecisionRecorder use case. healing.rs writes both legacy reason and
   Decision node during transition.
4. ADR-045 extension: validator rules for L7/L8 (conflict-participant-kind,
   causal-acyclic, conflict-min-participants).
5. CLI: `da conflicts list|resolve|escalate`, `da decisions record|trace`.

### Phase 2: Pipeline DSL + Policy

1. ADR-049 Phase 2: ParallelismManager, ResourceScheduler, paused state.
2. ADR-048 Phase 2: Policy node, rule engine, check_decision_rules.
3. ADR-046 Phase 2: validator warns on missing recorded_at; back-fill job.
4. W3C PROV-O export.

### Phase 3: Back-fill + enforcement

1. ADR-046 Phase 3: recorded_at required on new fact-bearing writes.
2. Schema version bump to 2 on L6/L7/L8/L9-fact-bearing schemas.
3. Migration plan via ADR-044 framework.

---

## 9. Summary

v2 ontology takes the four strongest Semantica patterns — bi-temporal
facts, conflict reification, decision intelligence, and pipeline DSL —
and adapts them to scientific KG semantics. It adds 2 schemas, 8 edges,
2 layers, and ~25 new triplet shapes. It preserves everything v1
materialized and unlocks five new query patterns that v1 could not answer.

The design is layered (L0–L10), directional (layer dependency invariant),
bi-temporal where it matters (fact-bearing nodes only), and auditable
(every system action becomes a Decision with a causal chain). The
validator (ADR-045) gains five new rules; the pipeline (ADR-049) gains
pre-flight and failure escalation; the CLI gains six new subcommands.

Open questions are scoped in each ADR. Phase 1 deliverables are
chunkable into the existing slice/wave workflow.
