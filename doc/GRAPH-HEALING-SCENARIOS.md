# Graph Healing Scenarios

**Status:** Binding (D135)
**Date:** 2026-07-26
**Depends on:** D132 (schema), D133 (ontology alignment), D134 (retrieval_eligible)

The graph must evolve: extraction errors need correction, duplicate entities
need merging, deprecated taxonomy needs migration, bad data needs silencing.
This document defines the healing operations.

---

## Scenario catalog

### 1. CORRECT — Fix a wrong property

**Problem:** Entity label "GPT" should be "GPT-4", or entity_type is wrong.

**Operation:** `correct(vid, key, old_value, new_value, reason)`
- Set property to new_value
- Log ProvenanceEvent (who, when, why, old→new)
- No new node — just property update

**Graph effect:** node property changes. Bi-temporal: old version gets
`valid_to=now`, new version is appended to history.

**CLI:** `da heal correct --vid <vid> --key label --value "GPT-4" --reason "typo"`

### 2. MERGE — Fuse duplicate entities

**Problem:** Two Entity nodes ("Transformer" and "Transformers") are the same.

**Operation:** `merge(vid_keep, vid_merge, reason)`
- vid_merge gets `retrieval_eligible=false` + `superseded_by=vid_keep`
- All edges pointing to vid_merge are redirected to vid_keep
- SUPERSEDES edge: vid_merge → vid_keep
- ProvenanceEvent logged

**Graph effect:** one node silenced + superseded, edges redirected.
Reversible: set retrieval_eligible=true on vid_merge, remove SUPERSEDES.

**CLI:** `da heal merge --keep <vid_a> --merge <vid_b> --reason "same entity"`

### 3. SPLIT — One entity becomes two

**Problem:** Entity "BERT" was used for both the model and the paper title.

**Operation:** `split(vid, new_label, new_type, edges_to_move, reason)`
- Create new Entity node (new_vid)
- Move specified edges from vid to new_vid
- Both nodes remain retrieval_eligible=true
- SPLITS edge: vid → new_vid
- ProvenanceEvent logged

**Graph effect:** new node created, some edges redirected.

**CLI:** `da heal split --vid <vid> --new-label "BERT (paper)" --new-type Method`

### 4. SILENCE — Deprecate/quarantine a node

**Problem:** Extraction was wrong, or data is deprecated (e.g., old Concepts).

**Operation:** `silence(vid, reason)`
- Set `retrieval_eligible=false`
- Optionally set `deprecated_reason` property
- All retrieval queries automatically exclude it (D134 pattern)
- ProvenanceEvent logged

**Graph effect:** node stays in graph (for audit) but excluded from retrieval.

**CLI:** `da heal silence --vid <vid> --reason "wrong extraction"`

### 5. MIGRATE — Ontology version transition

**Problem:** OpenAlex Concepts → Topics migration.

**Operation:** `migrate_taxonomy(old_label, new_label, mapping: [(old_vid, new_vid)])`
- For each (old, new): create MIGRATED_TO edge old → new
- Old nodes: retrieval_eligible=false
- New nodes: retrieval_eligible=true
- All Works with hasConcept edges get parallel hasTopic edges
- HierarchySnapshot node records the migration version

**Graph effect:** parallel taxonomy edges, old silenced, new live.

**CLI:** `da heal migrate --from Concept --to Topic --mapping mappings.jsonl`

### 6. ROLLBACK — Revert extraction to previous version

**Problem:** New extractor version produced worse results.

**Operation:** `rollback_extraction(paper_vid, to_version)`
- All Entity nodes created by extractor_version > to_version get
  retrieval_eligible=false
- Entities from to_version get retrieval_eligible=true (if they were
  silenced by the newer run)
- ProvenanceEvent logged

**Graph effect:** selective silencing/un-silencing by extractor_version.

**CLI:** `da heal rollback --paper-vid <vid> --to-version 2`

### 7. REPAIR CITES — Fix wrong citation edges

**Problem:** Paper A doesn't actually cite Paper B (GROBID parsing error).

**Operation:** `repair_cites(source_vid, target_vid, action, reason)`
- action=remove: silence the CITES edge (set edge property confidence=0.0)
- action=retarget: change edge target to correct vid
- ProvenanceEvent logged

**CLI:** `da heal cites --source <vid> --target <vid> --action remove`

---

## Common patterns

### Provenance audit trail

Every healing operation creates a `ProvenanceEvent`:
```json
{
  "operation": "merge",
  "actor": "human:username | agent:sona | system:extractor",
  "timestamp": "2026-07-26T12:00:00Z",
  "affected_vids": ["vid:entity:...", "vid:entity:..."],
  "reason": "same entity, different surface forms",
  "before": { ... },
  "after": { ... }
}
```

### Bi-temporal guarantees

No data is ever deleted. Corrections create new versions:
- Old version: `valid_to=now`, `superseded_by=new_vid`
- New version: `valid_from=now`, `valid_to=None`
- `as_of(when)` query returns the state at any point in time

### Reversibility

| Operation | Reversible? | How? |
|-----------|:-----------:|------|
| correct | Yes | supersede back |
| merge | Yes | un-silence vid_merge, remove SUPERSEDES |
| split | No | cannot undo edge redistribution cleanly |
| silence | Yes | set retrieval_eligible=true |
| migrate | Yes | switch retrieval_eligible back |
| rollback | Yes | re-enable silenced entities |
| repair_cites | Partially | retarget is reversible, remove is not |

### SUPERSEDES edge type

```cypher
(:Entity {vid: "vid:entity:Method:Transformer"})
  -[:SUPERSEDES {reason: "merge", timestamp: ...}]->
(:Entity {vid: "vid:entity:Method:Transformers"})
```

### SPLITS edge type

```cypher
(:Entity {vid: "vid:entity:Method:BERT"})
  -[:SPLITS {reason: "ambiguous label", timestamp: ...}]->
(:Entity {vid: "vid:entity:Method:BERT (paper)"})
```

---

## Implementation plan

**Phase 1 (this session):**
- da-domain: HealingOperation enum, ProvenanceEvent struct
- da-application: GraphHealingUseCase (correct, merge, silence)
- da-graph: healing queries (find superseded, find duplicates)

**Phase 2 (next):**
- CLI: `da heal` commands
- Edge redirect in SamyamaGraphStore
- Bi-temporal property versioning

**Phase 3 (future):**
- Automated duplicate detection (string similarity + embedding similarity)
- Rollback by extractor_version
- Taxonomy migration tooling
