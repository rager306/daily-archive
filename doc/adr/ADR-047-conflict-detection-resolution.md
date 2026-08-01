# ADR-047: Temporal Edge Invalidation (revised — simplifies Conflict approach)

**Status:** Proposed (revised — supersedes Conflict node draft)
**Date:** 2026-07-24 (revised)
**Deciders:** collaborative
**Related:**
- ADR-046 (temporal edge model — provides the primitives this ADR uses)
- ADR-042 (Claim/EvidenceBundle)
- ADR-045 (validator)
- ADR-048 (Decision Intelligence — records invalidation decisions)
- ADR-050 (kg-* crate family)
- Research: Graphiti `resolve_edge_contradictions` + `resolve_extracted_edge`

## Context

The initial draft of this ADR introduced a `Conflict` node type for
**all** disagreements between facts — factual, typological, temporal,
and metric. Analysis of Graphiti's production system revealed that
**edge-level temporal invalidation** handles 80% of these cases
without a separate Conflict node, and does so more simply.

Graphiti's `resolve_extracted_edge` + `resolve_edge_contradictions`
pattern: when a new temporal edge arrives between the same endpoints
as an existing edge, compare their `valid_at`/`invalid_at` windows.
If the new edge supersedes the old, set `old.invalid_at = new.valid_at`
and `old.expired_at = now()`. The old edge remains in the graph
(history preserved) but is no longer "active" for queries.

This is the **same operation** our original ADR-047 Conflict node
performed, but without the ceremony of creating a node, two
CONFLICTS_OVER edges, a resolution strategy enum, and a RESOLVED_BY
edge. The temporal fields on the edges themselves carry the resolution.

### What remains for Conflict node

Edge invalidation handles **bilateral** contradictions (new fact vs
old fact, same relationship). It does NOT handle:

- **Multi-party disputes**: 3+ sources asserting different values for
  the same fact simultaneously (e.g., three papers reporting different
  accuracy for the same method on the same dataset).
- **Credibility-weighted resolution**: when sources have different
  reliability tiers and the system must pick a winner based on source
  credibility, not temporal order.
- **Explicit human escalation**: when the system cannot auto-resolve
  and must flag for expert review.
- **Legal domain normative conflicts**: hierarchical conflict between
  federal law and regional law, or between statute and constitution.
  These require jurisdiction-aware resolution, not temporal.

These remain as future work (ADR-047 Phase 2 or a follow-up ADR).

## Decision

### Phase 1: Edge-level temporal invalidation (this ADR)

Implement temporal edge resolution in kg-algorithms, inspired by
Graphiti's `resolve_edge_contradictions`. No Conflict node.

#### Algorithm

```text
resolve_temporal_edges(new_edge, existing_edges):
  invalidated = []
  for old in existing_edges:
    # Rule 1: old was already invalid before new became valid → skip
    if old.invalid_at is not None and new.valid_at is not None
       and old.invalid_at <= new.valid_at:
      continue

    # Rule 2: new was invalid before old became valid → skip
    if new.invalid_at is not None and old.valid_at is not None
       and new.invalid_at <= old.valid_at:
      continue

    # Rule 3: old is strictly earlier → new supersedes
    if old.valid_at is not None and new.valid_at is not None
       and old.valid_at < new.valid_at:
      old.invalid_at = new.valid_at
      old.expired_at = utc_now()
      invalidated.append(old)
      continue

    # Rule 4: temporal overlap and neither strictly earlier → retain both
    # (multi-version period, transition period in law, competing measurements)
    # Do nothing — both edges remain active.
  return invalidated
```

#### Placement

| Concern | Crate | Module |
|---|---|---|
| `resolve_temporal_edges` function | kg-algorithms | `temporal::resolution` |
| Edge property updates (invalid_at, expired_at) | kg-storage | `traits::DirectGraphStore` |
| "find existing edges between these endpoints" | kg-storage | `traits::get_edges_between(source, target, edge_type)` |

#### New port method needed in kg-storage

```rust
/// Get all edges of a given type between two nodes.
/// Used by temporal resolution to find potentially-contradicting edges.
async fn get_edges_between(
    &self,
    source: u64,
    target: u64,
    edge_type: &str,
) -> Vec<EdgeRecord>;
```

Where `EdgeRecord` carries edge_id + all temporal properties.

#### When invalidation runs

After every new temporal edge write:
1. Pipeline writes new edge (SUPPORTS, REFUTES, CITES, REFERENCES, etc.)
2. Pipeline calls `resolve_temporal_edges(new_edge, existing_edges)`
3. For each invalidated old edge: write `invalid_at` + `expired_at`
4. Optionally: emit a Decision node (ADR-048) recording the
   invalidation (`category: "edge_invalidated"`, `scenario:
   "temporal overlap resolution"`)

Step 4 is optional — invalidation can happen silently (Graphiti does)
or with an audit trail (recommended for legal/compliance domains).

#### What this replaces from the original ADR-047

| Original ADR-047 element | Status in revised ADR |
|---|---|
| Conflict node type | **Deferred** (Phase 2, future ADR) |
| CONFLICTS_OVER edge | **Deferred** |
| RESOLVED_BY edge | **Deferred** |
| 7 resolution strategies | **Simplified** to 4 rules (skip/skip/supersede/retain-both) |
| Conflict kinds (factual/typological/temporal/metric) | **Covered by temporal resolution** for bilateral; multi-party deferred |

### Phase 2: Conflict node (future, separate ADR)

When multi-party disputes need explicit resolution:

- Conflict node returns, but ONLY for 3+ source disagreements.
- Resolution strategies (voting, credibility_weighted, expert_review)
  apply to Conflict nodes, not to individual edges.
- Edge invalidation handles the bilateral case; Conflict handles the
  multi-party case.

This is intentionally deferred. The edge invalidation from Phase 1
is sufficient for the first legal KG deployment (bilateral
amendment/repeal resolution).

## Compatibility with ADR-046

ADR-046 (revised) defines the **5-field temporal edge model**.
This ADR defines the **invalidation algorithm** that operates on those
fields. They are complementary:

- ADR-046: what temporal fields exist on edges.
- ADR-047: how those fields are used to resolve contradictions.

## Alternatives considered

### 1. Keep original ADR-047 Conflict node for everything

Rejected. Graphiti demonstrates that edge-level invalidation is
sufficient for the majority of temporal contradictions. Creating a
Conflict node + 2 CONFLICTS_OVER edges + resolution strategy for
every bilateral amendment (extremely common in legal domain) is
excessive ceremony. Defer Conflict to multi-party cases.

### 2. No invalidation — just write new edges and let old ones linger

Considered. Simple but wrong: queries return stale facts alongside
current ones. "What is the current penalty for Article 105?" would
return all historical penalties unless the query explicitly filters
by `invalid_at IS NULL`. Edge invalidation makes the default query
correct.

### 3. Delete old edges on invalidation

Rejected. Destroys history. Legal and scientific domains need to
answer "what was the law on date X" — requires historical edges with
their temporal windows intact.

## Consequences

### Positive

- **Simpler than Conflict node** — 4 resolution rules vs 7 strategies
  + node/edge lifecycle.
- **Production-proven** — Graphiti uses exactly this pattern at scale.
- **Sufficient for legal domain** — bilateral amendment/repeal is the
  most common case; multi-party conflicts are rare.
- **History-preserving** — invalidated edges remain queryable for
  point-in-time queries.
- **Composable with ADR-048** — invalidation can optionally emit a
  Decision for audit trail.

### Negative

- **Conflict node deferred** — multi-party disputes (3+ sources) have
  no native representation until Phase 2. Workaround: use a Decision
  node with `category: "multi_source_dispute"` + reference all
  involved edges in `scenario` text.
- **New port method needed** — `get_edges_between(source, target,
  edge_type)` must be added to DirectGraphStore trait and implemented
  in SamyamaGraphStore + MockGraphStore.

## Open questions

1. **Should invalidation be synchronous (blocking pipeline) or
   asynchronous (background job)?** Tentative: synchronous for single-
   edge writes (common case); async batch for bulk ingest.
2. **Retain-both default for overlapping windows** — is this correct
   for all domains? Legal transition periods need it; scientific
   metric disagreements need it (different measurements coexist).
   Tentative: yes, retain-both is the safe default.
3. **Should Decision emission be mandatory or optional on
   invalidation?** Tentative: optional, configurable via pipeline
   policy. Legal/compliance domains will want it mandatory.
