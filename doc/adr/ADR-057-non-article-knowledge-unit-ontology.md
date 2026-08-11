# ADR-057: Non-Article Knowledge Unit Ontology (Textbook / Chapter / Curriculum)

**Status:** Proposed  
**Date:** 2026-08-11  
**Deciders:** collaborative  
**Related:**
- ADR-050 (ontology as DATA — kg-ontology sole universal authority)
- ADR-053 (article multi-layer document model — **article-shaped** only)
- ADR-055 (ACAP — assurance; candidate vs authoritative)
- ADR-056 §2.11 (load plane admits non-article **catalog** units only;
  forbids inventing paper↔chapter graph edges in the loader)
- D127 (import gate locked)
- D220 (ADR-056 Accepted — universal load plane)
- FaBiO / FRBR (`hasPart` / `isPartOf`) already used for Paper→Section

## 1. Context

### 1.1 Product fact

The corpus is not only scientific papers. Operators already load:

- **GNN textbook** — 28 HTML units under `source_code=gnn_textbook`
  (`da load --batch-toc …`, hub content optional)
- Stanford notes / other HTML textbooks (`source_profile=textbook`)
- Future: PubMed fulltext HTML, lecture notes, curated KB chapters

Load plane (ADR-056) is **done** for ensure + content materialize of these
units as process-local `Paper`-shaped nodes (HTML primary → HtmlParser →
sections/entities). That is **catalog + content readiness**, not a designed
curriculum graph.

### 1.2 The gap

Today after `da load … gnn-ch-chapters__06… --stages ingest+extract`:

| What exists | What is missing |
|-------------|-----------------|
| Catalog unit + HTML primary | Typed **Book** / **Chapter** identity distinct from Paper |
| Process-local sections + entities | Edges **chapter → entity/concept** with curriculum semantics |
| LoadReport `container_ref` field (string) | Durable graph link Paper↔Chapter (shared Entity) |
| `hasPart` Paper→Section | `hasPart` Book→Chapter (different FRBR level) |
| Entity MENTIONS from chapter text | Policy: same Entity as paper surface vs namespaced |

Without this ADR, any “helpfulness” in the loader or hub batch that writes
`CITES` / `hasPart` / invents Book nodes is **ontology smuggling** and will
fight ontology v2 DATA-driven governance.

### 1.3 Why now

- Load plane no longer blocks acquisition (28/28 GNN chapters on disk).
- Hub content can produce a multi-chapter snap — agents will ask for
  “chapter 6 methods vs papers that use GCN”.
- ADR-056 open point #7 explicitly deferred this design **before** graph
  promotion of textbook structure.

## 2. Decision

### 2.1 Binding split (do not collapse)

| Layer | Owner | Graph truth? |
|-------|--------|--------------|
| **Catalog / load** (ADR-056) | `ArticleLoadUseCase` | **No** — assets + LoadReport only |
| **Content materialize** (current) | Ingest/Extract as Work-like | Process-local only; D127 locked |
| **Non-article ontology** (this ADR) | DATA + domain types | **After Accept + materialize wave** |
| **Promotion** | ordered mutation + D127 | Only under typed contracts |

### 2.2 Node families (DATA-first, FaBiO-aligned)

Introduce **additive** labels (do not overload `Paper` forever):

| Label | Role | Stable id sketch |
|-------|------|------------------|
| `Collection` | Book / course / curated corpus container | `vid:collection:{source_code}:{slug}` |
| `KnowledgeUnit` | Chapter / lecture / about page (load unit) | `vid:ku:{source_code}:{unit_key}` |
| `CurriculumPath` (optional later) | Ordered reading path | deferred |

**Reuse where honest:**

- `Section` remains document structure **inside** a unit (Paper or KnowledgeUnit).
- `Entity` / `Method` / `Dataset` / … remain the **shared concept surface** —
  same label across paper and textbook when identity folds (ADR-resolution).
- Do **not** invent `TextbookPaper` hybrids.

**Migration stance:** until materialize wave lands, content pipeline may still
write `Paper` nodes for HTML chapters (current behavior). ADR Accept does
**not** require an immediate dual-write. A later slice maps
`source_profile=textbook` → `KnowledgeUnit` (+ `Collection` parent).

### 2.3 Edge vocabulary (proposed contracts)

All new edges: `temporal_class: structural` unless noted. Wire strings
DATA-driven in `data/ontology/edge_contracts.yaml` (not hardcoded forever).

| Edge | Source → Target | Meaning | Notes |
|------|-----------------|---------|-------|
| `hasPart` | Collection → KnowledgeUnit | Book contains chapter | Extend contract endpoints beyond Paper→Section |
| `isPartOf` | KnowledgeUnit → Collection | Inverse | Optional explicit inverse vs query reverse |
| `hasPart` | KnowledgeUnit → Section | Chapter contains section | Same wire string, multi-endpoint contract |
| `MENTIONS` | KnowledgeUnit → Entity\|Method\|… | Unit mentions concept | Same as Paper MENTIONS |
| `foundIn` | Entity → Section | Extraction grounding | Unchanged |
| `RELATED_PAPER` (name TBD) | KnowledgeUnit → Paper | Scholarly link (cites/discusses) | **fact**; candidate-only until evidence |
| `SHARES_ENTITY` | — | **Not an edge** | Use shared Entity identity + resolution |

**Forbidden from loader / ensure:**

- Silent `CITES` between chapter and paper
- Auto curriculum trees without TOC DATA
- Promoting OpenAlex/Crossref enrichment as textbook structure

### 2.4 Identity & shared Entity

1. **Collection** identity comes from TOC DATA
   (`data/source_catalogs/*_toc.yaml`: `container`, `source_code`, `base_url`).
2. **KnowledgeUnit** identity = versionless `unit_key` under `source_code`
   (already catalog path).
3. **Entity** extracted from chapter text uses the **same** Entity plane as
   papers; fold via existing resolution (ADR entity_resolution.yaml) —
   not a parallel “textbook entity” namespace.
4. Cross-surface links Paper↔KnowledgeUnit require **evidence** (shared
   entity, explicit citation, or human Decision) — not string title match alone.

### 2.5 Graph readiness states

Align with LoadReport / ACAP:

| State | Meaning |
|-------|---------|
| `not_claimed` | Catalog/content only (default after load) |
| `structure_candidate` | Collection/KU nodes drafted, D127 locked |
| `structure_authoritative` | Promoted under D127 unlock + contracts |
| `rejected` | Failed validation / policy |

Loader stamps **`not_claimed` only**. Claiming higher states is a future
materialize use case, not `da load`.

### 2.6 Materialize waves (after Accept — not this ADR’s code)

| Wave | Deliverable |
|------|-------------|
| N1 | DATA: node schemas + edge_contracts for Collection/KnowledgeUnit |
| N2 | Domain types + vids; kg-ontology registry load |
| N3 | Application: `NonArticleStructureUseCase` (TOC → candidate graph) |
| N4 | Map HTML ingest path: KnowledgeUnit instead of/in addition to Paper |
| N5 | Graph tasks + acceptance fixtures for textbook hub |
| N6 | Governor consumes structure report (after product fixtures) |

### 2.7 Explicit non-goals

- Opening D127.
- Rewriting ADR-053 article tree (PageIndex) in this ADR.
- Forcing PDF primary for textbooks.
- LLM curriculum generation.
- Replacing Entity with SKOS Concept scheme (may align later).

## 3. Consequences

### Positive

- Clear boundary: load remains universal; ontology stays DATA-driven.
- GNN textbook hub content can grow without polluting Paper semantics.
- Shared Entity plane enables real paper↔chapter questions later.

### Negative / cost

- Temporary dual model: chapters may still be Paper nodes until N4.
- `hasPart` multi-endpoint contracts need careful validator updates.
- Acceptance suite must expand beyond paper-only tasks.

### Risks

| Risk | Mitigation |
|------|------------|
| Inventing edges in load “for demo” | ADR-056 + this ADR forbid; governor later |
| Overloading Paper | Explicit KnowledgeUnit migration wave |
| Hardcoded labels | YAML node_schemas + edge_contracts only |

## 4. Validation (when implemented)

- Contract: Collection/KnowledgeUnit schemas load via OntologyRegistry.
- Integration: TOC → candidate structure snap; D127 false.
- UAT: LoadReport `graph_readiness=not_claimed` after `da load`; structure
  use case separate command/report.
- Negative: load path must not create Collection edges (test).

## 5. Status action

**Proposed** for review. Upon **Accepted**:

1. Index + GSD decision.
2. Milestone for waves N1–N3 (DATA → domain → candidate structure use case).
3. Do **not** change `da load` architecture; only metadata stamps and later
   optional structure command.

Until Accept: product may stamp LoadReport `container_ref` / `source_profile`
from TOC (string metadata only) — already allowed by ADR-056 §2.11.
