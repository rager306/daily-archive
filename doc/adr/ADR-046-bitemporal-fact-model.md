# ADR-046: Temporal Edge Model — Bi-Temporal Validity on Edges

**Status:** Proposed (revised — supersedes node-centric bi-temporal draft)
**Date:** 2026-07-24 (revised)
**Deciders:** collaborative
**Related:**
- ADR-040 (Samyama schemaless sole store)
- ADR-042 (Claim/EvidenceBundle — now edges, not just nodes)
- ADR-045 (validator — gains edge-temporal rules)
- ADR-047 (conflicts — simplified by edge invalidation)
- ADR-050 (kg-* crate family — temporal lives in kg-ontology + kg-algorithms)
- Research: [Graphiti](https://github.com/getzep/graphiti) (getzep/graphiti, arXiv:2501.13956),
  [OntoKG](https://arxiv.org/html/2604.02618v1),
  [Semantica](https://github.com/semantica-agi/semantica),
  [Neo4j GRAPH TYPE](https://neo4j.com/blog/developer/graph-type-schema-enforcement-made-easy-preview/)

## Context

The initial draft of this ADR placed bi-temporal fields (`valid_from`,
`valid_to`, `recorded_at`, `superseded_at`) on **fact-bearing nodes**
(Claim, EvidenceBundle, MetricObservation). Analysis of Graphiti's
production temporal model revealed this is the wrong layer:

### Why edges, not nodes

**Graphiti's design** (validated at scale, arXiv:2501.13956): temporal
fields live on `EntityEdge` — the fact/relationship between entities.
Nodes (Entity, EpisodicNode) represent **persistent things** that do
not have validity windows. Edges represent **facts** that become true,
stop being true, and can be invalidated by newer facts.

This maps to three independent traditions that converged on the same
answer:

1. **Property Graph theory** (Neo4j GQL ISO/IEC 39075): relationships
   are first-class records with their own property maps. Temporal
   validity is a relationship property, not a node property.
2. **RDF-star / RDF 1.2 reification**: statements about statements.
   "Fact X was true from 2020 to 2023" is a triple about a triple —
   modelled as edge metadata.
3. **Knowledge Graph survey** (Hogan et al, ACM Computing Surveys 2021):
   a KG is "a data graph enriched with a schema, identity, and context."
   Temporal context attaches to assertions (edges), not to entities
   (nodes).

### What this means concretely

In daily-archive / kg-* core:

- `Paper MENTIONS Entity` — the **MENTIONS edge** carries temporal
  validity ("this paper mentioned this entity, extracted on date X").
  The Paper and Entity themselves are persistent.
- `EvidenceBundle SUPPORTS Claim` — the **SUPPORTS edge** carries
  validity ("this evidence supported this claim from date A to date B").
  The Claim persists; the support relationship may be superseded.
- `Decision CAUSED Decision` — the **CAUSED edge** carries temporal
  scope ("decision A caused decision B, established at time X").
- In legal domain: `Article_v1 REFERENCES Article_v2` — the
  **REFERENCES edge** carries validity ("this cross-reference held
  from 1996 to 2011"). The articles themselves persist as nodes.

### Node-level temporality still exists (but is secondary)

Nodes still carry **provenance timestamps** (not validity windows):

| Node field | Meaning | On nodes? | On edges? |
|---|---|---|---|
| `created_at` | When node/edge was first written | yes | yes |
| `valid_at` | When fact became true in real world | EpisodicNode only | **yes (primary)** |
| `invalid_at` | When fact stopped being true | — | **yes (primary)** |
| `expired_at` | When system invalidated this record | — | **yes (primary)** |
| `reference_time` | Timestamp from source episode | EpisodicNode | **yes** |

Nodes = "this thing exists." Edges = "this relationship holds from X to Y."

## Decision

### 1. Five-field temporal model on edges

Every temporal edge (SUPPORTS, REFUTES, MENTIONS, REFERENCES, CITES,
CAUSED, INFLUENCED, and domain-specific fact edges) carries:

| Field | Type | Required | Description |
|---|---|---|---|
| `valid_at` | DateTime | yes | When the fact became true in the real world |
| `invalid_at` | DateTime? | optional | When the fact stopped being true (`null` = still true) |
| `expired_at` | DateTime? | optional | When the system invalidated this edge (`null` = active) |
| `reference_time` | DateTime? | optional | Timestamp from the source that produced this edge |
| `created_at` | DateTime | yes (auto) | When the system first wrote this edge (transaction time) |

Two temporal axes:
- **Valid time** (valid_at → invalid_at): when the fact was true in the real world.
- **Transaction time** (created_at → expired_at): when the system knew it.

### 2. EpisodicNode as provenance ground truth

Adopt Graphiti's EpisodicNode pattern. Every raw source (paper, law,
court decision, JSON feed) is an EpisodicNode with:

| Field | Type | Description |
|---|---|---|
| `vid` | String | Stable identifier |
| `source_type` | String | `message` / `text` / `json` / `xml` / `legal_act` / `court_decision` |
| `source_description` | String | Human-readable source label |
| `content` | String | Raw content (ground truth) |
| `valid_at` | DateTime | When the source was originally created (publication date, decision date) |
| `created_at` | DateTime | When we ingested it |
| `episode_metadata` | Map? | Domain-specific metadata for filtering |

Every temporal edge stores `reference_time` from its source EpisodicNode.
This creates a **bi-temporal provenance chain**:
edge.valid_at → EpisodicNode.valid_at → source publication date.

### 3. Edge invalidation (replaces Conflict node for 80% of cases)

When a new temporal edge overlaps an existing edge between the same
endpoints, the old edge is **invalidated** (not deleted):

```text
New edge:    valid_at=2024-01-01, invalid_at=null
Old edge:    valid_at=2020-01-01, invalid_at=null

→ Old edge gets: invalid_at=2024-01-01, expired_at=now()
```

Rules (adapted from Graphiti `resolve_edge_contradictions`):

1. If old.invalid_at ≤ new.valid_at → old was already invalid before new → skip.
2. If new.invalid_at ≤ old.valid_at → new was invalid before old → skip.
3. If old.valid_at < new.valid_at → new supersedes old:
   old.invalid_at = new.valid_at, old.expired_at = now().
4. If temporal windows overlap and neither is strictly earlier →
   **retain both** (multi-version period, e.g. transition periods in
   law).

This is simpler than the ADR-047 Conflict node for bilateral
disagreements. Conflict node remains for **multi-party** disputes
(3+ sources disagreeing) and cases needing explicit resolution strategy
selection.

### 4. Temporal query primitives

kg-storage gains temporal query builders (Cypher WHERE clause generators):

- `active_at(date)` — edges where valid_at ≤ date AND (invalid_at IS NULL OR invalid_at > date)
- `known_at(date)` — edges where created_at ≤ date AND (expired_at IS NULL OR expired_at > date)
- `valid_during(start, end)` — edges with temporal window overlapping [start, end]
- `superseded_by(edge_id)` — chain of edges that invalidated this one

These live in kg-storage as query helpers, not in application code —
so any project (scientific, legal, enterprise) gets temporal search
for free.

### 5. Retroactivity support (optional, domain-specific)

For domains where facts can have retroactive effect (law, accounting):

| Field | Type | Description |
|---|---|---|
| `retroactive_to` | DateTime? | If set, the fact applies retroactively from this date |
| `retroactivity_basis` | String? | Legal/operational basis for retroactivity |

These are **optional edge properties** — not part of the core 5-field
model. Projects that don't need retroactivity ignore them. The
`is_active_at(date)` helper checks `retroactive_to` if present:

```text
is_active_at(edge, date):
  effective_start = edge.retroactive_to ?? edge.valid_at
  effective_end = edge.invalid_at ?? OPEN
  return effective_start ≤ date < effective_end
```

### 6. kg-* crate placement

| Concern | Crate | Module |
|---|---|---|
| Temporal edge type definitions | kg-ontology | `temporal` |
| 5-field model, retroactivity, EpisodicNode schema | kg-ontology | `schema` |
| `is_active_at`, `was_known_at`, `is_current` | kg-ontology | `temporal` |
| `invalidate_edge`, `resolve_edge_contradictions` | kg-algorithms | `temporal` |
| Temporal query builders (Cypher WHERE) | kg-storage | `temporal_query` |
| Edge property get/set for temporal fields | kg-storage | `traits` |

kg-ontology defines the **types and predicates**.
kg-storage provides the **query and storage primitives**.
kg-algorithms implements the **resolution logic**.

### 7. Node-level temporality (simplified)

Nodes carry only **provenance timestamps**, not validity windows:

| Node type | Temporal fields |
|---|---|
| EpisodicNode | `valid_at`, `created_at` (provenance of the source) |
| Entity (Paper, Person, Article, Concept) | `created_at` only |
| Decision | `created_at`, `valid_at` (when decision took effect) |

Entities persist. Only their **relationships** (edges) become true/false
over time. This is the Graphiti model and it simplifies node schemas
dramatically — no more `valid_from`/`valid_to` on Claim, EvidenceBundle,
MetricObservation. Those fields move to the edges connecting them.

## Alternatives considered

### A. Keep node-level bi-temporal (original ADR-046 draft)

Rejected. Confuses entity persistence with fact validity. A Claim
node "Claim X is true" persists even after it's refuted — what changes
is the SUPPORTS edge's `invalid_at`. Putting validity on the node
means either deleting the node (losing history) or having a confusing
"valid_to" on something that still exists.

### B. RDF-star reification for everything

Considered. RDF-star (`<<:alice :worksFor :acme>> :validFrom "2020"`)
is the W3C-standard way to express edge metadata. Rejected for now
because our storage (Samyama) is a property graph, not a triple store,
and property graphs support edge properties natively without
reification ceremony. If we later need RDF interop, the edge
properties can be serialized to RDF-star.

### C. Event-sourced temporal log

Considered. Append-only log of temporal events, replay to determine
state at time T. Rejected for query complexity — every "what was true
on date X" requires replaying all events up to X. Edge-validity fields
answer the same question in O(1) per edge.

## Consequences

### Positive

- **Aligns with production-proven model** (Graphiti, Neo4j GRAPH TYPE,
  RDF-star).
- **Simplifies node schemas** — no more valid_from/valid_to on 8+
  fact-bearing node types. Temporal lives on edges only.
- **Enables temporal search** natively — `active_at(date)` query
  builder in kg-storage, available to any project.
- **Supports legal domain** — retroactive_to, transition periods
  (overlap_allowed via retain-both rule), versioned articles.
- **Edge invalidation is simpler** than Conflict node for 80% of
  cases. Conflict node reserved for genuine multi-party disputes.

### Negative

- **Breaking change from ADR-046 draft** — Claim, EvidenceBundle,
  MetricObservation lose their valid_from/valid_to/recorded_at/
  superseded_at fields; those move to SUPPORTS/REFUTES/PARTICIPATES_IN
  edges.
- **Edge property writes increase** — every temporal edge now gets 3-5
  property writes instead of 1-2. Storage cost negligible but pipeline
  code must set them.
- **Query patterns change** — "find all active claims" becomes "find
  all SUPPORTS edges where invalid_at IS NULL" rather than "find all
  Claim nodes where valid_to IS NULL."

### Migration

Not needed yet — ADR-046 Phase 1 (node-level bi-temporal fields) was
declared but **never populated in the pipeline**. No existing data
has these fields. The migration is purely a design revision.

## Open questions

1. **Should EpisodicNode be a separate node type in kg-ontology, or
   an aspect applied to any node?** Tentative: separate type — it has
   a distinct shape (content + source_type) that generic nodes don't.
2. **How many temporal fields on edge?** Core 5 (valid_at, invalid_at,
   expired_at, reference_time, created_at) + optional 2 (retroactive_to,
   retroactivity_basis). Total 7 max, 5 minimum.
3. **Overlap resolution default**: retain-both (Graphiti rule 4) vs
   latest-wins. Tentative: retain-both for safety; domain config can
   override.
4. **Should Conflict node (ADR-047) be downgraded from Phase 1 to
   Phase 2?** Tentative: yes. Edge invalidation covers the majority
   case; Conflict node is over-engineering for the first pass.
