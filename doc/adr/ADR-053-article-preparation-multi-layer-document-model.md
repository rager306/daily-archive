# ADR-053: Article Preparation 2.0 — Multi-Layer Document Model for Agent Readiness

**Status:** Proposed
**Date:** 2026-08-05
**Deciders:** collaborative
**Related:**
- ADR-040 (Samyama sole KG+vector+persist store — schemaless, Cypher)
- ADR-041 (embedded Cypher + HOT path)
- ADR-042 (query-local evidence activation over reified EvidenceBundle)
- ADR-043 (Research Process Plane — Execution-Grounded Scientific Memory)
- ADR-048 (Decision Intelligence — Decision node records each extraction/merge/cross-paper claim; this ADR wires it into production)
- ADR-045 (schema validator invariants)
- ADR-046 (bi-temporal edge model — extended to nodes in this ADR; all new fact edges follow M327–M329 stamping)
- ADR-047 (conflict detection — cross-paper edges are primary conflict source; new conflict_policy field in edge_contracts.yaml)
- ADR-050 (kg-* crate family — ontology as DATA)
- ADR-051 §8.1 (semantic graph task suite — case-study DoD)
- D127 (import gate — locked until explicit user yes)
- D137 (6-Wave KB Roadmap; this ADR feeds Wave 2 retrieval + Wave 4 agent brain)
- Reference: `/root/vendor-source/scientific-agent-skills` (BGPT structured extraction, paper-lookup, hypothesis-generation, experimental-design, literature-review)
- Reference: `/root/vendor-source/semantica` (graph-native infrastructure for context and accountable AI; entity resolution, decision intelligence, PROV-O provenance, SHACL/SKOS ontology governance, bi-temporal fact, agent context bundles; see "Semantica alignment" section)
- Prior Python-era concept: PageIndex (cold-archived `archive/package-rename-waves/wave-04/src/arxiv_archive/indexing/page_index.py`)

## Context

The current Rust pipeline parses PDFs through GROBID into a **flat list of
`Section { title, text, level }`** structs. Section nodes become Samyama graph
nodes linked by `hasPart` edges, and `RuleBasedExtractor` emits `Entity` nodes
linked by `MENTIONS`. Extraction eval on 193 papers reaches Micro F1 = 0.869,
and the GEPA hub graph task suite passes 7/7. By extraction metrics, the
current model is sufficient.

But this model has **four structural gaps** that block agent navigation and
graph-algorithm readiness:

1. **Flat structure, no navigation tree.** Section has no stable path id, no
   parent/child/sibling links, no depth-aware traversal. GROBID TEI is natively
   nested (`<div><head n="1">…</head><p>…</p></div>`), but the Rust adapter
   flattens this into a list and discards the tree. An agent cannot ask
   "Methods subsection 3.2 setup paragraph" because the structural address is
   gone. The Python-era `PageIndex` (`archive/package-rename-waves/wave-04`)
   represented each section as a `PageIndexNode` with `parent_id`,
   `children_ids`, `next_id`, `path`, `provenance` — and provided
   `find_by_title`, `children_of`, `path_to`, `walk_next`. That concept was
   **intentionally dropped** during the Rust rewrite for pipeline simplicity
   but its loss removes document-environment navigation.

2. **No summary layer.** A Summary is not a first-class node. Every downstream
   task (retrieval, comparison, agent reasoning) must re-read full text. There
   is no cheap entry point: no tldr, no per-section summary, no methods digest,
   no results digest, no limitations digest, no reproducibility digest. The
   Reference `scientific-agent-skills` BGPT skill returns 25+ structured fields
   per paper (methods, results, sample sizes, quality scores) — the daily-archive
   graph has none of these as queryable objects.

3. **Mono-Entity.** `Entity` is one label with an `entity_type` string field.
   There is no first-class typing for `Method`, `Dataset`, `Baseline`, `Metric`,
   `Hyperparameter`, `Hardware`, `Task`. Retrieval cannot filter "show me the
   datasets used" vs "show me the methods proposed" without text matching on
   `entity_type`. Graph algorithms (WCC, SCC, page rank) operate on a uniform
   `Entity` population and cannot exploit type-homogeneous subgraphs.

4. **No reading paths.** Only three structural edges exist today:
   `hasPart`, `CITES`, `MENTIONS`. An agent has no pre-computed route through
   the graph for typical tasks: critical reading, reproducibility check,
   baseline comparison, citation lineage, novelty scan, evidence chain. ADR-051
   §8.1 already defines the **semantic graph task suite** as parameterized
   Cypher templates — but the current templates operate on the thin existing
   edge set and cannot express reading paths that cross summary + typed entity
   + section layers.

### Why now

- Wave 2 (retrieval pipeline, D137 main bottleneck) needs richer per-paper
  objects to retrieve against — not only Section text.
- Wave 4 (agent brain SymFSM + SONA) explicitly needs document navigation
  (the original purpose of Python-era PageIndex and RLM document navigation).
- D127 keeps the import gate closed, so this is the right moment to design the
  **durable shape** of an article *before* production writes are authorized.
- `scientific-agent-skills` confirms the industry pattern: structured paper
  fields (BGPT), typed results (experimental-design), hypothesis-evidence
  chains (hypothesis-generation), thematic synthesis (literature-review).

### What is NOT changing

- **Pipeline shape:** PDF → GROBID TEI → graph stays. We do **not** reintroduce
  a PDF→markdown→chunk pipeline. The layered model is **additive** on top of
  GROBID-native parsing.
- **D127:** all new nodes and edges carry `import_eligible = false` until
  independent review and explicit user yes. Nothing in this ADR opens the gate.
- **Ontology as DATA (ADR-050 Phase D):** new node types and edges land in
  `data/ontology/node_schemas.yaml` and `data/ontology/edge_contracts.yaml`,
  not as hardcoded Rust.
- **Extraction is still rule-based.** D136 keeps DSPy/LLM-assisted extraction
  locked until eval metrics + benchmark fixtures are verified. This ADR adds
  **targets** for extraction (typed entities, structured summaries) — the
  extraction mechanism stays rule-based initially and may later use LLM helpers
  behind D136 governance.

## Decision

Adopt a **six-layer article preparation model** that makes a paper a navigable,
multi-granularity, typed, path-ready object in the graph. Layers are additive:
every layer can be built independently and the absence of a layer never breaks
existing queries.

### Layer A — Document Tree (Section becomes navigable)

Section gains structural address fields and navigation edges, restoring
PageIndex-equivalent navigation in Rust on top of GROBID-native TEI.

| Addition | Source |
|----------|--------|
| `path_id` (e.g. `3.2.1`) on Section | Computed deterministically from GROBID TEI nested `<div>` structure at parse time |
| `parent_section_id`, `order_in_parent`, `depth` on Section | Derived from TEI nesting |
| Navigation edges `next_sibling`, `first_child`, `parent_section` (structural, `created_at` only per ADR-046) | New edge contracts in `edge_contracts.yaml` |
| Structural validation diagnostics at ingest | New validator rules in `da-domain/src/validator.rs` (duplicate path, orphan section, broken parent link, depth inconsistent with path_id) |

**Result:** Section is a navigable tree. Agent can ask
"MATCH (s:Section {path_id:'3.2'})-[:first_child]->(sub) RETURN sub".

### Layer B — Summary nodes (multi-granularity digests)

`Summary` is a new first-class node type with a `summary_type` discriminator.
A paper has multiple summaries at multiple granularities; each summary links
to its evidence sections.

| `summary_type` | Content | Source / trigger |
|----------------|---------|------------------|
| `abstract` | tldr, key_claim, contributions list | Extracted from paper abstract at ingest |
| `section` | per-section digest | Rule-based heuristic initially; LLM later under D136 |
| `methods` | method_type, datasets, baselines, metrics, setup | Specialized extraction (rule patterns + later LLM) |
| `results` | quantitative findings, comparisons, ablations | Specialized extraction |
| `limitations` | stated limitations, threats to validity | Specialized extraction |
| `reproducibility` | code_available, data_available, hyperparams, hardware, seed | Specialized extraction |

Edges:
- `Paper` `hasSummary` `Summary` (structural)
- `Section` `summarized_by` `Summary` (structural)
- `Summary` `evidence_from` `Section` (structural; multi-target; preserves traceability)

**Principle:** compute once, use many times. Each downstream task starts by
reading summaries, not full text.

### Layer C — Typed Entity subtypes

`Entity` becomes an abstract base. Concrete subtypes are first-class labels,
queryable and filterable, enabling type-homogeneous subgraph views.

| Subtype label | Examples | Retrieval use |
|---------------|----------|---------------|
| `Method` | LoRA, GNN, RLHF, novel architecture | "what methods are proposed?" |
| `Dataset` | MMLU, ImageNet, custom | "what was evaluated on?" |
| `Baseline` | SOTA, prior art, random | "compared against what?" |
| `Metric` | F1, BLEU, accuracy@k | "how is it measured?" |
| `Hyperparameter` | learning rate, batch size | "what config?" |
| `Hardware` | A100×8, TPU-v4 | "what is required?" |
| `Task` | classification, retrieval | "what problem?" |

Edges:
- `Method` `evaluated_on` `Dataset`
- `Method` `compared_to` `Baseline`
- `Metric` `observed_in` `MetricObservation` (already in ontology)
- `Entity` subtype `MENTIONS` stays, with the typed source giving richer queries

**Migration:** existing `Entity` nodes are preserved. New labels are added in
`node_schemas.yaml`. The extractor is extended to classify entities into
subtypes; unclassified entities remain `Entity`.

### Layer D — Reading Paths (parameterized Cypher templates)

Reading paths are **not hardcoded graph edges**. They are parameterized Cypher
templates registered in the ADR-051 §8.1 graph task suite. Each template
describes a route across layers A–C for a typical agent task.

| Path template | Route | Use case |
|---------------|-------|----------|
| `critical_reading` | Abstract → ResearchProblem → Method → Results → Limitations | Fast paper understanding |
| `reproducibility_check` | Method → Setup → Hyperparams → Code/Data availability | Can it be reproduced? |
| `baseline_comparison` | Method → `compared_to` → Baseline → ResultComparison | What is beaten and by how much? |
| `citation_lineage` | Paper → `CITES` → Paper (with Abstract) | Who builds on whom |
| `novelty_scan` | Abstract → Related Work → Method delta | What is new vs prior art |
| `evidence_chain` | Claim → EvidenceBundle → `evidence_from` → Section span | Can the claim be trusted? |

Each template is a `.tasks.cypher` file under `data/graph_tasks/` and is
covered by the existing semantic graph task suite runner. **A path exists only
when its dependencies (layers A–C) are present in the graph.**

### Layer E — Quality Signals

`QualitySignal` is a node attached to `Paper` via `hasQualitySignal`. Each
signal is a computed annotation, **never a blocking gate**.

| Signal | Formula | Source |
|--------|---------|--------|
| `reproducibility_score` | weighted: code + data + hardware + seed | ReproducibilitySummary |
| `novelty_signals` | new Method/Baseline delta vs Related Work | novelty_scan path output |
| `evidence_density` | grounded claims / section count | EvidenceBundle count |
| `completeness_score` | filled summary types / total summary types | Summary layer coverage |
| `readiness_for_agents` | composite: tree valid + summaries present + typed entities | All layers |

Signals prioritise papers for agent attention. Example:
`MATCH (p:Paper)-[:hasQualitySignal]->(q:QualitySignal {name:'evidence_density'}) WHERE q.value > 5 RETURN p`.

### Layer F — Cross-paper Patterns (graph-algorithm-ready edges)

Typed entities and structured summaries make meaningful cross-paper edges
possible, giving `kg-algorithms` (page rank, WCC, SCC, community detection)
non-trivial structure to operate on.

| Edge | From → To | Semantics |
|------|-----------|-----------|
| `predecessor_of` | Method → Method | Method lineage (temporal fact edge) |
| `superseded_by` | Baseline → Baseline | SOTA timeline (temporal fact edge) |
| `derived_from` | Dataset → Dataset | Data provenance |
| `replicates` | Paper → Paper | Replication studies |
| `contradicts` | Paper → Paper | Disagreement graph |

All cross-paper edges are `temporal_class: fact` (ADR-046) — they can be
invalidated. Cross-paper edge creation is a **separate milestone** and is
gated by evidence (not free invent, D127 spirit).

## Temporal policy per layer (ADR-046 / ADR-047 alignment)

Article Preparation 2.0 adds nodes and edges whose **lifecycle is not
necessarily eternal**. The bi-temporal edge model (ADR-046) and conflict
detection (ADR-047) apply to every new type. The policy below makes the
temporal class explicit per layer so that ingest, extraction, and enrichment
stamping helpers (M327–M329 patterns) cover the new types without ad-hoc
rules.

### Per-edge temporal class

| Edge | Layer | temporal_class | Rationale |
|------|-------|----------------|----------|
| `next_sibling`, `first_child`, `parent_section` | A | `structural` | Document composition — only `created_at`; no invalidation race (TEI structure is immutable per PDF hash) |
| `hasSummary` (Paper → Summary) | B | `structural` | Point-in-time link to current summary; supersede handled on Summary node (see below) |
| `summarized_by` (Section → Summary) | B | `structural` | Composition link |
| `evidence_from` (Summary → Section) | B | `structural` | Provenance — only `created_at` |
| `evaluated_on` (Method → Dataset) | C | `fact` | A method may be re-evaluated on a different dataset; supersede via valid_at/invalid_at |
| `compared_to` (Method → Baseline) | C | `fact` | Comparison can be retracted; full TemporalEdge |
| `hasQualitySignal` (Paper → QualitySignal) | E | `structural` | Link to current snapshot; supersede handled on QualitySignal node |
| `predecessor_of` (Method → Method) | F | `fact` | Lineage claim can be invalidated (wrong attribution, later correction) — EvidenceBundle required |
| `superseded_by` (Baseline → Baseline) | F | `fact` | SOTA timeline — new evidence can refute the claim |
| `derived_from` (Dataset → Dataset) | F | `fact` | Provenance claim can be corrected |
| `replicates` (Paper → Paper) | F | `fact` | Replication claim; retractable |
| `contradicts` (Paper → Paper) | F | `fact` | Disagreement claim; retractable |

All `fact` edges follow the M327–M329 stamping pattern: shared
`temporal_stamp` helper, `is_temporal_edge_type` lookup in
`edge_contracts.yaml`, `create_edge_with_temporal` write path, EvidenceBundle
attached as evidence of the claim.

### Node-level bi-temporality (extends ADR-046 to nodes)

Some new node types are **computed snapshots** whose content changes over
time. ADR-046 currently defines bi-temporality on edges; this ADR extends the
same model to two node types:

| Node type | Why temporal | Lifecycle fields |
|-----------|-------------|------------------|
| `Summary` | Re-extraction produces new content; old version should remain queryable for audit | `source_hash`, `generated_at`, `valid_from`, `valid_to`, `superseded_by` |
| `QualitySignal` | Recomputed after re-ingest or re-extract; old snapshot applies to a different reference_time | `reference_time`, `valid_from`, `valid_to`, `superseded_by`, `computation_version` |

Supersede protocol (identical for both node types):

1. New node is created with `valid_from = now`, `valid_to = null`.
2. Previous active node for the same `(paper_id, summary_type)` or
   `(paper_id, signal_name)` is loaded.
3. Previous node is stamped: `valid_to = now`, `superseded_by = new_vid`.
4. `hasSummary` / `hasQualitySignal` edge is rewritten to point at the new
   node. Old edges remain for audit; queries filter on `valid_to IS NULL`
   (or use the current active edge).

This mirrors the M327–M329 edge-supersede pattern but on nodes, and reuses
the `created_at` stamping infrastructure from M331 (structural created_at).

### Typed Entity classification is a fact edge

`Entity` subtype promotion (e.g. an `Entity` reclassified from generic to
`Method` after a better extraction pass) is **not** a node mutation. It is a
`classified_as` fact edge (`Entity` → `EntityType` shadow node) OR a node
supersede (old `Entity` `valid_to`, new `Method` node `valid_from`, same
`vid` stem). The edge formulation is preferred because it composes cleanly
with the existing bi-temporal model and keeps the audit trail explicit.

| Edge | From → To | temporal_class | Rationale |
|------|-----------|----------------|----------|
| `classified_as` | Entity → Method/Dataset/Baseline/... | `fact` | Classification can be revised; full TemporalEdge |

**Migration note:** initial Wave 1 S02 extraction creates `Method`/`Dataset`/
etc. as **direct labels** (simpler, faster retrieval) without an `Entity`
origin. The `classified_as` edge is reserved for the **re-classification**
case in later waves when an existing `Entity` is promoted to a subtype.

### Conflict detection (ADR-047) hooks

Cross-paper fact edges (`predecessor_of`, `superseded_by`, `replicates`,
`contradicts`) are the **most likely sources of conflicts**:

- Paper A claims `Method X predecessor_of Method Y`; Paper B claims the
  8opposite. ADR-047 conflict detection fires on the `(source, target, type)`
  signature.
- `contradicts` is itself a conflict signal. The conflict resolver may treat
  reciprocal `contradicts` edges as "ongoing disagreement" (not an error) but
  treat `predecessor_of` + `predecessor_of` reverse as a hard conflict.

`Conflict` node (existing ontology) is used unchanged for these cases. The
new edge contracts declare their conflict policy in
`edge_contracts.yaml` (field `conflict_policy`: `hard` / `soft` /
`reciprocal_ok`), consumed by the M329 conflict resolver extension.

### Governor and observability hooks

- `is_temporal_edge_type` lookup in `edge_contracts.yaml` is extended for the
  new `fact` edges. The M329 governor rule for `temporal_stamp` presence on
  fact edges covers the new types automatically.
- A new governor rule (Wave 1 S03) verifies that `Summary` and
  `QualitySignal` nodes with `valid_to IS NULL` have exactly one
  `hasSummary`/`hasQualitySignal` edge active — blocks stale-but-current
  snapshots.
- Tracing spans (ADR-052) on `temporal_stamp` calls include the new edge
  types so audit log covers cross-paper claims.

## Semantica alignment (methodology cross-check)

Review against `/root/vendor-source/semantica` (graph-native infrastructure for
context and accountable AI) surfaced **six capabilities** that the original
six-layer model above does not address explicitly. This section declares how
Article Preparation 2.0 relates to each — either covered (by which layer),
extended (new work), or consciously out of scope (with rationale).

### 1. Entity Resolution and Deduplication — NEW LAYER G

The original six layers introduce typed entities (Layer C) and cross-paper
edges (Layer F) but never answer: when two papers mention "LoRA" is that
the same Method node? Semantica treats this as a first-class pipeline stage
( semantica.deduplication + semantica.entity_resolution ): blocking by
fingerprint, semantic similarity, governed merge with provenance-preserving
links. Without it, typed entities fragment into per-paper duplicates and
Layer F lineage becomes impossible (you cannot draw predecessor_of between
"LoRA" in paper A and "LoRA" in paper B if they are two separate nodes).

**Add Layer G — Entity Resolution and Merge:**

| Object | Purpose |
|--------|---------|
| EntityEquivalence edge (fact, temporal) | Links two Entity nodes declared the same real-world object |
| merge_decision Decision node (uses ADR-048) | Records the merge act: who/what/why/confidence/evidence |
| Entity.canonical_vid field | Points to the surviving canonical entity after merge |
| BlockingIndex operational artifact | Fingerprints (lowercased label + type + arxiv-key) for candidate generation |

Merge protocol (mirrors Semantica EntityMerger):
1. Blocking — cheap fingerprint index surfaces candidate pairs.
2. Semantic match — embedding similarity + property overlap (no free invent;
   confidence below threshold stays as candidate, never silent merge).
3. Merge decision recorded as a Decision node (ADR-048 wiring).
4. Surviving entity gains canonical_vid; merged entities get valid_to +
   superseded_by (bi-temporal node pattern). Edges re-pointed; originals
   retained for audit.

**Principle:** duplicates are flagged, not silently overwritten (Semantica
compliance principle; aligns with D127 spirit).

Layer G is Wave 1 because Layer F cross-paper edges depend on resolved
canonical entities.

### 2. Decision Intelligence per extraction step — extends Layer B/C

Semantica records every consequential decision as a first-class object with
causal chain and precedent search ( semantica.context.decision ). daily-archive
has ADR-048 (Decision node) and ADR-043 (Research Process Plane) but neither is
wired into extraction. Article Preparation 2.0 changes this:

| Decision type | Recorded when | Evidence |
|---------------|---------------|----------|
| classification | Entity promoted to Method / Dataset / etc. | source span + confidence |
| summary_generation | Summary node created or superseded | source_hash + extractor version |
| merge (Layer G) | Two entities resolved to one | blocking fingerprint + similarity score |
| cross_paper_claim (Layer F) | predecessor_of / contradicts edge created | cited text span |
| quality_signal (Layer E) | Signal value computed | computation_version + inputs |

Each decision links to its evidence via Decision.evidence_bundle and to the
resulting node/edge via decided_by edge. This gives ADR-048 a concrete
production footprint (currently ADR-048 is schema-only) and answers Semantica's
regulator question for every extraction step.

### 3. Context Graph for agents — NEW LAYER H (agent context)

Semantica's ContextGraph / AgentContext is the queryable, bounded view an
agent operates on for a task. ADR-053 Layer D adds reading-path templates, but
a reading path is only a route — the agent still needs a materialized,
versioned, scope-bounded context bundle to reason over. This is the
working-memory object that Wave 4 agent brain (D137) consumes.

**Add Layer H — Agent Context Bundle:**

| Object | Purpose |
|--------|---------|
| ContextBundle node (bi-temporal) | Frozen view of N nodes + M edges relevant to a task |
| scope field | Cypher snippet that produced the bundle (reproducible) |
| as_of field | Point-in-time reference (aligns with Semantica time-travel) |
| task_kind field | critical_reading / reproducibility_check / compare_methods / ... |
| compiled_at field | When the bundle was frozen |

A bundle is read-only (agent cannot mutate graph through it), bounded
(deterministic size budget), and reproducible (same scope + as_of re-derives
the same view). This is the Semantica AgentContext pattern adapted to
daily-archive's bi-temporal Samyama store.

### 4. Provenance — W3C PROV-O export (extends Layer B evidence_from)

EvidenceBundle and evidence_from capture internal provenance. Semantica
exports W3C PROV-O (regulator-ready audit trail). daily-archive should be able
to export any node/edge's full provenance chain as PROV-O Turtle/JSON-LD on
demand. This is a Wave 3 export target, not a new node type: the data is
already in Source, EvidenceBundle, Decision, and the bi-temporal fields;
PROV-O is the standard serialization.

| Output format | When |
|---------------|------|
| PROV-O Turtle | da export-provenance --id <vid> --format turtle |
| PROV-O JSON-LD | da export-provenance --id <vid> --format jsonld |
| CSV audit | batch export per Paper (regulator pack) |

PROV-O mapping (illustrative):
- Entity (PROV) ← any node (Paper, Section, Summary, Method, ...)
- Activity ← extraction / ingest / merge / decision events
- Agent ← Source / extractor version / human reviewer
- wasGeneratedBy, wasDerivedFrom, wasAttributedTo, used ← existing FROM_SOURCE,
  evidence_from, decided_by, hasSummary

### 5. Ontology governance — SHACL + SKOS (strengthens ADR-050)

Semantica ships SHACL constraints, OWL generation, and SKOS vocabulary
management with a visual editor. ADR-050 keeps ontology as DATA in YAML but
does not expose a constraints/vocabulary layer. Article Preparation 2.0 adds
node types and edges that need declarative constraints beyond field types
(e.g. Method must have at least one evaluated_on edge or ReproducibilitySummary
code_available=true requires a code_url field).

**Wave 2 governance extension:**

| Capability | Source of truth | Used by |
|------------|-----------------|---------|
| SHACL-style constraints | data/ontology/constraints.yaml | Validator (da-domain) |
| SKOS vocabularies | data/ontology/vocab/{domain}.skos.yaml | Typed Entity classification |
| OWL export | da ontology-export --format owl | External reasoners |
| Cross-ontology alignment | data/ontology/alignments/*.yaml | FaBiO/CiTO/OpenAlex crosswalk |

Constraints are non-blocking warnings initially (governor does not fail
trajectory); promoted to errors once fixtures stabilise (mirrors M343 pattern
for serde roundtrip regression prevention).

### 6. Deterministic reasoning — Rete / Datalog over the graph

Semantica runs deterministic forward chaining and Datalog over the KG without
an LLM. daily-archive has kg-algorithms (page_rank, WCC, SCC, BFS, Dijkstra,
PPR) but no rule engine. This is the most ambitious Semantica-aligned
capability and is declared out of scope for Article Preparation 2.0 with a
clear hook:

- The ContextBundle (Layer H) is the natural input to a future rule engine.
- Typed entities (Layer C) + cross-paper edges (Layer F) give the rule engine
  non-trivial facts to chain over.
- A future ADR (Wave 5 of D137, post-agent-brain) will specify the rule engine
  (Rete-style or Datalog-style) when the process plane (ADR-043) needs
  forward-chaining for hypothesis to prediction to observation checks.

Recording this as a conscious deferral keeps ADR-053 scoped and avoids
boiling the ocean.

### Cross-cutting principle: deterministic infrastructure layer

Semantica's central thesis — no LLM required for graph construction,
reasoning, or provenance — is already daily-archive's posture (D136 locks
DSPy/LLM extraction). The six layers (A-F) and the two new layers (G entity
resolution, H agent context) are all deterministic. LLM assistance may enter
later for summary generation (Layer B) and entity classification (Layer C),
always behind D136 governance and always recording a Decision node (item 2
above) so the LLM's contribution is auditable, never silent.

### Updated layer map (final)

| Layer | Capability | Wave | Semantica alignment |
|-------|-----------|------|---------------------|
| A | Document Tree | 1 | hierarchical chunking (semantica.split) |
| B | Summaries (bi-temporal nodes) | 1+2 | semantica.semantic_extract output objects |
| C | Typed Entities | 1 | semantica.NamedEntityRecognizer target schema |
| D | Reading Paths | 2 | semantica.ContextGraph route templates |
| E | Quality Signals (bi-temporal nodes) | 2 | semantica.context.PolicyEngine inputs |
| F | Cross-paper Patterns | 3 | semantica.kg.BiTemporalFact + conflicts |
| **G** | **Entity Resolution and Merge** | **1 (blocks F)** | **semantica.deduplication + entity_resolution** |
| **H** | **Agent Context Bundle** | **2 (Wave 4 input)** | **semantica.context.AgentContext** |
| **I** | **Figures, Tables, Datapoints** | **2** | **GROBID TEI figure/table/formula + semantica.semantic_extract output objects** |
| **J** | **Terminology, Glossary, Jargon** | **2** | **semantica.ontology.SKOS + concept layer** |
| Governance | SHACL/SKOS/OWL/PROV-O export | 2+3 | semantica.ontology + semantica.provenance |
| Out of scope | Deterministic reasoning engine | future ADR | semantica.reasoning (Rete/Datalog/SPARQL) |
| Out of scope (Wave 5+) | Product/Experience Layer | future **ADR-054** | personal corpus, alerts, synthesis UI, lab memory, writing integration (8 Tier-3 pains) |

## Pain point coverage (top-20 academic pains, empirical basis)

Cross-check against the aggregated top-20 researcher pains (Daniel 2022
n=62 postgraduates; CBE—Life Sciences Education; Nature Ecology & Evolution;
PLOS One; PRISMA-related surveys; 500+ researcher polls) calibrates which
layers the ADR should prioritize and where it is silent. Pains split into
three tiers:

**Tier 1 — covered by ADR-053 (infrastructure):** methods understanding
(#6 Layer B+C), cognitive load (#7 Layer B+D), big-picture structure
(#9 Layer A+D), reproducibility (#16 Layer B+E), partial critical appraisal
(#8 Layer B+E), partial citation tracking (#17 Layer F).

**Tier 2 — infrastructure gaps addressed by this revision:**

- #13 figures/tables/data interpretation → **Layer I** (new)
- #5 jargon/terminology → **Layer J** (new)
- #4 #17 citation context (supporting/contradicting) → **Layer F extension**
- #11 scope/coverage/gap → **Layer E extension**

**Tier 3 — product/experience layer; declared for future ADR-054** (not
built on top of thin air; consumes ADR-053 foundation): personal library +
semantic search (#3), continuous ingest + smart alerts + trend detection
(#1 #10), multi-paper synthesis output artifacts (#4 #15), collaborative
annotations + lab institutional memory (#14), critical appraisal workflow
+ RoB templates UI (#8), scope visualization + gap explorer UI (#11),
writing-tool integration (#15). Access/paywalls (#19) declared out of scope.

Cross-cutting principle: **ADR-053 ships the substrate; ADR-054 ships the
experience.** ADR-054 does not start until ADR-053 Wave 1-3 close — UI
without foundation re-creates the Python-era fragility.

## Layer I — Figures, Tables, and Datapoints (Tier 2 — pain #13)

Visual and quantitative artifacts become first-class extractable objects.
GROBID TEI natively marks `<figure type="image">`, `<figure type="table">`,
and `<formula>`; the daily-archive adapter currently discards them.

| Node | Source | Key fields |
|------|--------|-----------|
| `Figure` | GROBID TEI `<figure type="image">` | `label` (Fig. 3), `caption`, `figure_type` (chart/diagram/photo/schematic), `ref_section_ids` |
| `Table` | GROBID TEI `<figure type="table">` | `label` (Table 2), `caption`, `headers` (rule-extracted), `nrows`, `ncols` |
| `Datapoint` | rule-based over Tables | `metric_name`, `value`, `unit`, `condition`, `baseline_delta`, `evidence_from` |
| `Equation` | GROBID TEI `<formula>` | `latex`, `label` (Eq. 5), `context_section_id` |

Edges: `Paper` `hasFigure` `Figure`; `Figure` `describes` `Method`/`Dataset`
(Layer C typed); `Datapoint` `observes` existing `MetricObservation`;
`Figure` `evidence_from` `Section`.

All edges `temporal_class: structural` (created_at only). `Datapoint.observes`
uses fact semantics where the datapoint reports a comparison (Layer F tie-in).

Governor hook: `Figure`/`Table`/`Datapoint` added to `node_schemas.yaml`
and to the registered labels walk. Extraction is rule-based first; LLM-assisted
caption interpretation arrives later under D136.

## Layer J — Terminology, Glossary, and Jargon (Tier 2 — pains #5 #12 #18)

Terms and definitions become a navigable concept layer. Auto-built from the
existing typed-entity population (Layer C) plus entity resolution (Layer G);
grows as the corpus grows.

| Object | Purpose |
|--------|---------|
| `Term` node | Canonical term (`LoRA`, `Transformer`, `Markov chain`), `domain`, `first_seen_paper`, `aliases` |
| `GlossaryEntry` node | `definition`, `formality_level` (plain/technical/expert), `source_hash`, bi-temporal (supersedes on re-extract) |
| `defined_in` edge | `Section` → `Term` (where the term is defined in this paper) |
| `uses_term` edge | `Section` → `Term` (where the term is used) |
| `prerequisite_of` edge | `Term` → `Term` (concept hierarchy; typed entities feed this graph) |
| `translations` field on Term | Cross-domain mapping (e.g. “retrieval-augmented generation” CS ↔ “grounded generation” NLP); addresses pain #18 |

Progressive disclosure: agent query `define(term, level)` returns the matching
`GlossaryEntry`. Inline jargon explanation uses Layer H `ContextBundle`
expansion to attach definitions on demand.

Bi-temporal nodes: `GlossaryEntry` follows the same `valid_from` / `valid_to` /
`superseded_by` protocol as `Summary` and `QualitySignal` (extends ADR-046 to
nodes). `Term` nodes are structural (created_at only) — a canonical term does
not expire; its definitions may.

## Layer F extension — Citation Context (Tier 2 — pains #4 #17)

The existing `CITES` edge gains a `citation_context` property drawn from the
reference context window (the paragraph around the citation marker).

| `citation_context` value | Semantics | Maps to |
|--------------------------|----------|---------|
| `supporting` | Citing paper relies on cited | existing CITES |
| `contradicting` | Explicit disagreement | Layer F `contradicts` (fact edge) |
| `extending` | Builds on top | Layer F `predecessor_of` candidate |
| `replicating` | Replication study | Layer F `replicates` |
| `discussing` | Mention without position | existing CITES |
| `methodological_basis` | Uses the cited method | Layer C tie-in |
| `dataset_origin` | Uses the cited dataset | Layer C tie-in |

Rule-based extraction from citation context phrases ("as shown by", "in contrast
to", "building on", "we replicate"). Confidence field; below threshold stays
`discussing`. Decision node (Layer B/C decision intelligence extension) records
the classification with evidence span.

This is an **edge property**, not a new edge type. Existing `CITES` queries keep
working; the property is filterable: `MATCH (p)-[c:CITES]->(q) WHERE
.citation_context = 'contradicting' RETURN p, q`.

## Layer E extension — Coverage and Gap (Tier 2 — pain #11)

New signal types and a new node type detect research gaps and coverage.

| Object | Computation |
|--------|-----------|
| `coverage_score` QualitySignal | per concept / method: count of canonical entities + density of `predecessor_of` lineage |
| `trend_signal` QualitySignal | temporal slope: count of new `predecessor_of` edges over window |
| `GapNode` node | asserts "combination (Method X) × (Dataset Y) has zero `evaluated_on` edges in canonical corpus"; carries `EvidenceBundle` (completeness audit) + Decision recording |

GapNode lifecycle: created by gap-scan, **must be Decision-backed** (no free
invent — someone/something asserts the gap with evidence). GapNode can be
`superseded` (bi-temporal) when the gap is later filled; the fill event is
itself recorded.

## Product/Experience layer — ADR-054 (future, Tier 3)

Eight Tier-3 pains are product/experience, not infrastructure. ADR-054 will
specify them on top of ADR-053 foundation. Scoped here for traceability;
built only after ADR-053 Waves 1-3 close.

- Personal corpus + full-text semantic search + linked notes (pain #3)
- Continuous ingest feeds + smart alerts + trend detection (pains #1 #10)
- Multi-paper synthesis output artifacts: comparison tables, draft outlines,
  contradiction summaries (pains #4 #15)
- Collaborative annotations + lab institutional memory (pain #14)
- Critical appraisal workflow templates: RoB checklist, claim-evidence audit
  (pain #8)
- Scope visualization + gap explorer UI (pain #11)
- Writing-tool integration: export to Overleaf / Word / Obsidian / Pandoc
  (pain #15)
- Access/paywalls (pain #19): Unpaywall integration, OA prioritization —
  declared **out of scope** for daily-archive as a KB engine

ADR-054 design principles (locked here, detailed later):
- consumes ADR-053 Layers A-J via read-only `ContextBundle` (Layer H)
- never mutates the foundation graph directly
- every agent/person action records a Decision node (ADR-048 wiring)
- personal corpus is a filtered projection of the canonical KB, not a parallel
  store (single source of truth, ADR-040)

## Consequences

### Positive

- **Agent brain (Wave 4) becomes feasible.** Layer A restores document
  navigation; Layer D gives parameterized reading paths; Layers B+C give rich
  retrievable objects.
- **Retrieval (Wave 2) has richer targets.** Summaries and typed entities are
  first-class retrievable units, not only Section text.
- **Graph algorithms get meaningful subgraphs.** Typed entities (Layer C)
  enable type-homogeneous WCC/SCC; cross-paper edges (Layer F) enable
  non-trivial page rank and community detection.
- **Provenance is preserved end-to-end.** `evidence_from` (Layer B) +
  `foundIn` (existing) + `path_id` (Layer A) restore EvidencePath-equivalent
  traceability without reintroducing the Python chunk pipeline.
- **Incremental adoption.** Each layer is additive. Existing queries keep
  working. Layers can ship in separate milestones.

### Negative

- **More node types and edges to maintain.** Ontology grows from 31 to ~40
  node types; edge contracts grow by ~10. Schema versioning (ADR-044) must
  cover migrations.
- **More extraction work.** Typed entity classification and summary generation
  are new extraction tasks. Rule-based first; LLM-assisted later under D136.
- **Graph task suite grows.** Each reading path template is a new task file.
  ADR-051 §8.1 suite must stay green.
- **D127 governance.** All new nodes carry `import_eligible = false`. The
  governor rule `no_import_eligible_true_in_artifacts` (MEM642) applies to
  every new artifact type.

### Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Layer A path_id computation drifts on GROBID version bumps | Hash `path_id` from TEI structure, validate at ingest; pin GROBID version |
| Layer B summaries become stale vs paper text | Summary nodes carry `source_hash` + `generated_at`; re-extract on paper re-ingest |
| Layer C subtype classification errors pollute typed subgraphs | Confidence field on `Entity`; subtypes only promoted above threshold; ambiguous stays `Entity` |
| Layer D path templates bitrot as ontology evolves | Path templates are part of ADR-051 §8.1 suite — suite failure surfaces drift |
| Layer F cross-paper edges become free invent | Cross-paper edges require explicit evidence (citation, replication claim) — never inferred from text similarity alone |

## LLM Reading Notes

- This ADR is **design-only**. It introduces no code. Implementation lands in
  milestones scoped by the updated roadmap.
- The six layers are **independent** — each can be delivered, validated, and
  used without the others, although their value compounds when stacked.
- **Do not reintroduce chunk pipeline.** Layer A restores PageIndex-equivalent
  navigation on top of GROBID TEI; it does **not** recreate PDF→markdown→chunk.
- **Do not open D127.** Every new node type carries `import_eligible = false`.
  The governor rule on artifacts (MEM642) still applies.
- **Do not enable LLM extraction by the back door.** Layers B+C are targets
  for extraction. Initial extraction stays rule-based. LLM-assisted extraction
  follows D136 governance (eval metrics + benchmark fixtures first).
- **Temporal policy is explicit per layer.** Layer A navigation edges are
  `structural` (created_at only). Layers C/F typed-entity and cross-paper
  edges are `fact` (full 5-field TemporalEdge, M327–M329 stamping, retractable).
  Layers B/E Summary and QualitySignal nodes are **bi-temporal nodes**
  (valid_from/valid_to/superseded_by) — this ADR extends ADR-046 from edges to
  nodes for computed snapshots. Conflict detection (ADR-047) runs on
  cross-paper fact edges; `conflict_policy` declares hard/soft/reciprocal_ok.
- **Reference material:** `/root/vendor-source/scientific-agent-skills` BGPT
  skill confirms the industry direction (structured fields per paper);
  paper-lookup confirms per-source provenance (already covered by `Source`
  node); hypothesis-generation maps to the existing process plane
  (`Hypothesis` node, ADR-043); experimental-design informs the
  `MethodsSummary` schema (randomization, replication, blocking).
- **Wave mapping (D137):** Layer A unblocks Wave 4 agent brain; Layers B+C
  unblock Wave 2 retrieval; Layer F amplifies Waves 2/4/5.
- **No breaking changes to existing validated pipeline.** All additions are
  new labels/edges; no existing label or edge is removed.

## Open questions (deferred to milestone planning)

1. Should `Summary` nodes embed their text content or store only references to
   source sections? (Leaning: embed digest text; reference sections via
   `evidence_from` for full traceability.)
2. Should typed entity subtypes be separate labels (Cypher-fast) or a
   discriminator field on `Entity` (simpler schema)? (Leaning: separate labels
   for retrieval performance; migration path for existing `Entity` nodes.)
3. Should reading path templates (Layer D) be versioned with the ontology, or
   versioned independently as graph task suite entries? (Leaning: graph task
   suite entries — they are scenario files, not schema.)
4. Where does cross-paper edge evidence live? (Leaning: explicit
   `EvidenceBundle` per cross-paper edge; no similarity-based inference.)

## Supersession

None. This ADR **extends** ADR-040 (Samyama schemaless — new labels/edges are
data), ADR-042 (EvidenceBundle — summaries and typed entities carry evidence),
ADR-043 (process plane — summaries describe the publication plane, not the
research process plane), ADR-050 (ontology as DATA — new types land in YAML),
and ADR-051 §8.1 (graph task suite — reading path templates extend the suite).
It does not supersede any prior decision.
