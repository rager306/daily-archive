# ADR-053: Article Preparation 2.0 — Multi-Layer Document Model for Agent Readiness

**Status:** Proposed
**Date:** 2026-08-05
**Deciders:** collaborative
**Related:**
- ADR-040 (Samyama sole KG+vector+persist store — schemaless, Cypher)
- ADR-041 (embedded Cypher + HOT path)
- ADR-042 (query-local evidence activation over reified EvidenceBundle)
- ADR-043 (Research Process Plane — Execution-Grounded Scientific Memory)
- ADR-045 (schema validator invariants)
- ADR-046 (bi-temporal edge model — extended to nodes in this ADR; all new fact edges follow M327–M329 stamping)
- ADR-047 (conflict detection — cross-paper edges are primary conflict source; new conflict_policy field in edge_contracts.yaml)
- ADR-050 (kg-* crate family — ontology as DATA)
- ADR-051 §8.1 (semantic graph task suite — case-study DoD)
- D127 (import gate — locked until explicit user yes)
- D137 (6-Wave KB Roadmap; this ADR feeds Wave 2 retrieval + Wave 4 agent brain)
- Reference: `/root/vendor-source/scientific-agent-skills` (BGPT structured extraction, paper-lookup, hypothesis-generation, experimental-design, literature-review)
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
