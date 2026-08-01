# Graph Core Architecture — Temporal Edge Model

**Status:** Design (companion to ADR-046/047/050)
**Date:** 2026-07-24
**Scope:** kg-* crate family. Domain-specific details (scientific,
legal, enterprise) are out of scope — they live in application repos.

This document specifies the **temporal edge model** that sits at the
heart of the graph core. It synthesizes findings from Graphiti
(production temporal KG at scale), OntoKG (intrinsic-relational
routing), Neo4j GRAPH TYPE (schema enforcement philosophy), and
Semantic Web (RDF-star reification) into a single coherent design
for the kg-* crates.

---

## 1. Foundational Principle

**Entities persist. Facts change. Temporality lives on edges.**

```
Node = "this thing exists" (Paper, Person, Article, Statute, Concept)
Edge = "this relationship holds from X to Y" (SUPPORTS, REFERENCES, AMENDS)
```

Nodes carry identity + provenance timestamps only. Edges carry the
full temporal validity model. This is the Graphiti model, the RDF-star
model, and the property-graph-native model. It is not the model in
our earlier ADR-046 draft (which put temporality on nodes).

---

## 2. Temporal Edge Schema

Every edge that represents a **fact** (not a structural edge like
HAS_PART or FROM_SOURCE) carries temporal fields:

### Core 5 fields (required for all temporal edges)

| Field | Type | Required | Description |
|---|---|---|---|
| `valid_at` | DateTime | yes | When the fact became true in the real world |
| `invalid_at` | DateTime? | optional | When the fact stopped being true (null = still true) |
| `expired_at` | DateTime? | optional | When the system invalidated this edge (null = active) |
| `reference_time` | DateTime? | optional | Timestamp from the source episode |
| `created_at` | DateTime | auto | When the system wrote this edge |

### Optional extension fields (domain-specific, legal-aware)

| Field | Type | Description |
|---|---|---|
| `retroactive_to` | DateTime? | Fact applies retroactively from this date |
| `retroactivity_basis` | String? | Legal/operational basis for retroactivity |
| `overlap_allowed` | Bool? | If true, retain-both on temporal overlap (transition periods) |

### Which edges are "temporal"?

**Temporal** (carry 5-field model):
- SUPPORTS, REFUTES (evidence → claim)
- CITES (paper → paper/citation)
- REFERENCES (cross-reference between nodes)
- AMENDS, REPEALS, SUPERSEDES (versioning)
- CAUSED, INFLUENCED (decision → decision)
- MENTIONS (paper → entity — extraction timestamp matters)

**Non-temporal** (structural, carry only created_at):
- HAS_PART (paper → section — structural decomposition)
- FROM_SOURCE (paper → source — provenance tag)
- AUTHORED_BY (author → paper — persistent authorship)
- HAS_TOPIC, IN_CATEGORY (classification)
- MEMBER_OF_CLUSTER (community membership)

The distinction: **structural edges describe composition that doesn't
change over time**. Temporal edges describe **assertions that can
become false**.

---

## 3. EpisodicNode — Provenance Ground Truth

Every raw data source (paper, law, court decision, log entry) enters
the graph as an EpisodicNode:

| Field | Type | Description |
|---|---|---|
| `vid` | String | Stable identifier |
| `source_type` | String (enum) | `message` / `text` / `json` / `xml` / `legal_act` / `court_decision` |
| `source_description` | String | Human-readable label |
| `content` | String | Raw content (ground truth — never modified) |
| `valid_at` | DateTime | When the source was originally created |
| `created_at` | DateTime | When we ingested it |
| `episode_metadata` | Map? | Domain-specific filter keys |

**Provenance chain**: temporal edge → `reference_time` → EpisodicNode →
`valid_at` → source creation date. This chain answers "when did we
learn this fact, and from what source?"

**Back-references**: EpisodicNode stores a list of edge IDs that were
derived from it. This enables "show me all facts extracted from this
source" in one hop.

---

## 4. Temporal Resolution Algorithm

When a new temporal edge arrives between endpoints that already have
edges of the same type, the resolution algorithm runs:

```text
resolve_temporal_edges(new_edge, existing_edges) → invalidated_edges:

  for each old_edge in existing_edges:
    1. SKIP if old was already invalid before new became valid
    2. SKIP if new was invalid before old became valid
    3. SUPERSEDE if old is strictly earlier than new
       → old.invalid_at = new.valid_at; old.expired_at = now()
    4. RETAIN BOTH if temporal windows overlap and neither is strictly earlier
       → no action (transition period, competing measurements, dual versions)
```

This is a **4-rule temporal logic** adapted from Graphiti's production
`resolve_edge_contradictions`. It covers:

- Sequential versioning (law v1 → v2 → v3)
- Competing simultaneous facts (two papers, different accuracy)
- Transition periods (old law and new law both active for 6 months)
- Retractions (new source says old fact was wrong)

### What it explicitly does NOT cover

- Multi-party disputes (3+ sources, same fact, different values) →
  needs credibility-weighted resolution (Phase 2, future ADR)
- Hierarchical conflicts (federal vs regional law) → needs
  jurisdiction-aware resolution (Phase 2)
- Semantic deduplication (same fact phrased differently) → needs
  embedding similarity (Phase 2)

These remain in application-layer code or future kg-algorithms
extensions.

---

## 5. Temporal Query Primitives

kg-storage provides query builders for temporal scoping:

| Primitive | Cypher equivalent | Use case |
|---|---|---|
| `active_at(date)` | `WHERE e.valid_at <= $date AND (e.invalid_at IS NULL OR e.invalid_at > $date)` | "What was true on date X?" |
| `known_at(date)` | `WHERE e.created_at <= $date AND (e.expired_at IS NULL OR e.expired_at > $date)` | "What did the system know on date X?" |
| `valid_during(start, end)` | `WHERE e.valid_at < $end AND (e.invalid_at IS NULL OR e.invalid_at > $start)` | "What was true during this period?" |
| `superseded()` | `WHERE e.expired_at IS NOT NULL` | "What facts are no longer active?" |
| `current()` | `WHERE e.invalid_at IS NULL AND e.expired_at IS NULL` | "What is true right now?" |

These live in kg-storage as query-string builders (not runtime checks),
so they compose with Cypher MATCH/WHERE clauses in any graph backend.

**Retroactivity-aware variant**: if `retroactive_to` is set on the
edge, `active_at(date)` uses `retroactive_to` instead of `valid_at`
as the effective start date.

---

## 6. kg-* Crate Architecture (temporal-aware)

```
┌──────────────────────────────────────────────────────────────┐
│  kg-ontology                                                 │
│                                                              │
│  temporal::TemporalEdge — 5-field + optional extensions      │
│  temporal::is_active_at / was_known_at / is_current          │
│  temporal::EpisodicNodeSchema — provenance node type         │
│  schema::edge_type_registry — which edges are temporal       │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│  kg-storage                                                  │
│                                                              │
│  traits::get_edges_between(source, target, edge_type)        │
│  traits::set_edge_temporal_fields(edge_id, TemporalEdge)     │
│  temporal_query::active_at(date) → Cypher WHERE fragment     │
│  temporal_query::valid_during(start, end) → Cypher fragment  │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│  kg-algorithms                                               │
│                                                              │
│  temporal::resolve_temporal_edges(new, existing) → Vec<Edge> │
│  temporal::invalidate_edge(edge_id, at, store)               │
│  temporal::find_contradictions(label, store) → Vec<EdgePair> │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│  kg-pipeline                                                 │
│                                                              │
│  Pipeline stages call kg-algorithms after edge writes        │
│  TemporalInvalidationStage: runs after extract/enrich        │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

**Dependency flow**: ontology defines types → storage provides
primitives → algorithms implement resolution → pipeline orchestrates.

---

## 7. Domain-Agnostic Design Contract

The kg-* crates must work for **any temporal graph domain** without
domain-specific knowledge:

| Domain | EpisodicNode source | Temporal edges | What changes over time |
|---|---|---|---|
| Scientific KG (daily-archive) | Paper / Section | SUPPORTS, CITES, MENTIONS | Paper retraction, metric supersession |
| Legal KG | Statute / Court decision | AMENDS, REPEALS, REFERENCES | Law versions, precedent overruling |
| Enterprise KG | Document / Database record | WORKS_FOR, OWNS, CONTRACTED_WITH | Employment, ownership, contracts |
| Code KG (reactivegraph) | Git commit / PR | DEPENDS_ON, CALLS, IMPORTS | API changes, refactoring |

kg-* core does NOT know about:
- Paper, Claim, EvidenceBundle (scientific)
- Statute, Article, Court (legal)
- Employee, Contract (enterprise)
- Function, Class, Module (code)

It only knows:
- EpisodicNode (generic source)
- TemporalEdge (generic fact with validity window)
- resolve_temporal_edges (generic resolution)
- active_at / known_at (generic temporal queries)

**Domain mapping is the application's job.** The application repo
defines which node labels and edge types map to which kg-* primitives.

---

## 8. Legal Domain Validation Scenario

To confirm the design is legal-ready without adding legal-specific
code to kg-*:

### Scenario: Article versioning + amendment chain

```
EpisodicNode: "ФЗ-26 от 07.03.2011" (source_type=legal_act, valid_at=2011-03-07)

Article_105_v1 (Entity node)
  ← AMENDS (valid_at=2011-03-07, invalid_at=null) ← Article_105_v2 (Entity node)

Article_105_v2 supersedes Article_105_v1:
  AMENDS edge gets: old.invalid_at = 2011-05-01 (entry into force of amendment)
  resolve_temporal_edges rule 3: SUPERSEDE
```

### Scenario: Court decision invalidates norm

```
EpisodicNode: "Постановление КС РФ №x" (source_type=court_decision, valid_at=2020-07-20)

Norm_X (Entity node)
  ← INVALIDATED_BY (valid_at=2020-07-20, invalid_at=null) ← Court_Decision_Y (Entity node)

The INVALIDATED_BY edge is a temporal edge. Norm_X's existing SUPPORTS edges
from the original statute get invalid_at = 2020-07-20 via resolve_temporal_edges.
```

### Scenario: "What was the penalty on date X?"

```cypher
MATCH (a:Article)-[r:PENALTY]->(p:Penalty)
WHERE kg_storage.active_at(r, '2010-06-15')
RETURN p
```

The `active_at` temporal query primitive (kg-storage) handles this
without any legal-specific code. It just filters by valid_at/invalid_at.

### Scenario: Retroactive law

```
Norm_Z (Entity)
  ← APPLIES (valid_at=2024-01-01, retroactive_to=2020-01-01) ← Law_2024 (EpisodicNode)

Query: active_at('2021-06-15') → true (retroactive_to makes it active retroactively)
Query: active_at('2019-06-15') → false (before retroactive_to)
```

---

## 9. What's NOT in kg-* (application-layer concerns)

| Concern | Layer | Why |
|---|---|---|
| "Article" vs "Section" vs "Paragraph" labels | Application | Domain vocabulary |
| Jurisdiction hierarchy (federal/state/local) | Application | Domain model |
| Citation format parsing (ГОСТ, Bluebook) | Application | Domain parsing |
| Legal ontology alignment (CKAN, EuroVoc) | Application | Domain mapping |
| Court hierarchy | Application | Domain model |
| Specific resolution strategies (credibility-weighted) | Application | Domain policy |
| Document parsing (law text → structured data) | Application | Domain extraction |

kg-* provides: temporal edges, episodic provenance, resolution
algorithm, temporal queries. The application builds the legal KG
on top.

---

## 10. Summary

The temporal edge model is the **heart of the graph core**. It
provides:

1. **5-field temporal model on edges** (valid_at, invalid_at,
   expired_at, reference_time, created_at).
2. **EpisodicNode** as provenance ground truth with raw content.
3. **4-rule temporal resolution** (skip/skip/supersede/retain-both).
4. **Temporal query primitives** (active_at, known_at, valid_during).
5. **Optional retroactivity** for domains that need it.
6. **Domain-agnostic** — works for scientific, legal, enterprise, code.

The design is validated against Graphiti (production), RDF-star
(standard), and Neo4j GRAPH TYPE (vendor). It is ready for legal
domain deployment without adding legal-specific code to kg-* crates.
