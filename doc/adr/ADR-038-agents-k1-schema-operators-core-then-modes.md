# ADR-038: Agents-K1 Schema + Operators + Core-then-Modes

**Status:** Accepted (binding)
**Date:** 2026-07-25
**Deciders:** collaborative
**Amends:** ADR-037 (adds schema detail, extraction factorization, graph operators, tri-source)

> **ADR-040 updates:** Samyama Graph Cypher → Samyama Cypher. Schema structure (5 modules, 18 relations, CitationContext node) is unchanged — only DDL syntax changes. GraphStore trait backed by Samyama `EmbeddedClient`.
**Binding Level:** binding

---

## 0. One-line Decision

> We adopt the **Agents-K1 5-module knowledge schema** (A-E), **Core-then-Modes extraction factorization** (−50% LLM calls), **tri-source retrieval** (S_web + S_mmkg + S_kn), and **6 graph operators** (O1-O6) from the Shanghai AI Lab Agents-K1 paper (arXiv 2606.13669). These refine ADR-037's architecture without changing the hexagonal structure, RuVector/Samyama Graph split, or SymFSM agent control.

## 1. Why Agents-K1 matters

Agents-K1 (Shanghai AI Lab, June 2026) demonstrates that **knowledge structure**, not retrieval tuning, is the binding constraint for research agents. Key evidence:
- GPT-5.2 alone: 41.8% → **+ Agents-K1: 66.3%** on geoscience research (Δ +24.5pp)
- 4B model + GRPO outperforms 8B base on NER (closes 8× scale gap)
- Core-then-Modes cuts LLM calls **50%** at same output quality
- Scholar-KG: 2.46M papers, **MinerU parser = our OpenDataLoader** — validated parser choice

**MinerU IS OpenDataLoader.** Shanghai AI Lab's OpenDataLab produces both. Our ODL integration is the same parser used to build Scholar-KG.

---

## 2. Five-Module Knowledge Schema (replaces ADR-037 §5)

Samyama Graph schema expanded from 6 node types to **5 modules (A-E)**:

### Module A — Factual / Metadata

```ngql
-- Paper (existing, extended)
CREATE TAG IF NOT EXISTS Paper(
  -- identity
  vid string,                        -- SHA256 canonical ID
  arxiv_id string, doi string,
  title string, abstract string,
  pub_year int, venue string,
  paper_type string,                 -- journal / conference / preprint
  language string,
  peer_review_status string,
  license string, country string,
  pdf_url string, supplement_url string,
  -- temporality + versioning
  valid_from timestamp, valid_to timestamp DEFAULT 0,
  version int DEFAULT 1, superseded_by string DEFAULT '',
  -- quality signals
  citation_count int DEFAULT 0,
  journal_impact float DEFAULT 0.0,
  -- evidence + import
  evidence_ready bool DEFAULT false,
  import_eligible bool DEFAULT false
);

CREATE TAG IF NOT EXISTS Author(
  vid string, name string, canonical_name string,
  orcid string DEFAULT '', email string DEFAULT '',
  order int DEFAULT 0, affiliation_ror string DEFAULT ''
);

CREATE TAG IF NOT EXISTS Institution(
  vid string, name string, ror_id string, country string DEFAULT ''
);

CREATE TAG IF NOT EXISTS Resource(
  vid string,
  resource_type string,              -- repo / model / dataset / supplementary
  url string, hash string DEFAULT '',
  version string DEFAULT ''
);
```

### Module B — Textually Mentioned (entities found in text)

```ngql
CREATE TAG IF NOT EXISTS Task(
  vid string, label string, description string DEFAULT '',
  source_span_id string, confidence float DEFAULT 0.0,
  valid_from timestamp, valid_to timestamp DEFAULT 0, version int DEFAULT 1
);

CREATE TAG IF NOT EXISTS Method(
  vid string, label string, description string DEFAULT '',
  source_span_id string, confidence float DEFAULT 0.0,
  valid_from timestamp, valid_to timestamp DEFAULT 0, version int DEFAULT 1
);

CREATE TAG IF NOT EXISTS Dataset(
  vid string, label string, description string DEFAULT '',
  split string DEFAULT '', modality string DEFAULT '',
  source_span_id string, confidence float DEFAULT 0.0,
  valid_from timestamp, valid_to timestamp DEFAULT 0, version int DEFAULT 1
);

CREATE TAG IF NOT EXISTS Model(
  vid string, label string, description string DEFAULT '',
  parameters string DEFAULT '',       -- e.g. "4B", "7B"
  source_span_id string, confidence float DEFAULT 0.0,
  valid_from timestamp, valid_to timestamp DEFAULT 0, version int DEFAULT 1
);

CREATE TAG IF NOT EXISTS Metric(
  vid string, label string, long_name string DEFAULT '',
  acronym string DEFAULT '', formula string DEFAULT '',
  source_span_id string, confidence float DEFAULT 0.0,
  valid_from timestamp, valid_to timestamp DEFAULT 0, version int DEFAULT 1
);

CREATE TAG IF NOT EXISTS Baseline(
  vid string, label string, source_span_id string, confidence float DEFAULT 0.0
);

-- Implementation detail nodes (hardware, training config)
CREATE TAG IF NOT EXISTS ImplementationDetail(
  vid string,
  detail_type string,                 -- hardware / batch_size / lr / scheduler / epochs / seeds / amp / adapters
  value string,
  source_span_id string
);

-- Multimodal evidence nodes (NOT just text!)
CREATE TAG IF NOT EXISTS Figure(
  vid string, caption string,
  page int, bbox string,              -- layout coordinates
  artifact_hash string                -- PDF/ODL hash
);

CREATE TAG IF NOT EXISTS Table(
  vid string, caption string,
  page int, bbox string,
  cell_structure string,              -- JSON serialized cells
  artifact_hash string
);

CREATE TAG IF NOT EXISTS Equation(
  vid string, latex string,
  page int, bbox string,
  artifact_hash string
);

CREATE TAG IF NOT EXISTS Theorem(
  vid string, statement string, proof string DEFAULT '',
  source_span_id string
);

CREATE TAG IF NOT EXISTS Definition(
  vid string, term string, body string,
  source_span_id string
);
```

### Module C — Implicit / Abstracted (LLM-extracted, the "why")

```ngql
CREATE TAG IF NOT EXISTS ProblemDefinition(
  vid string,
  input_vars string,                  -- X
  output_vars string,                 -- Y
  constraints string,                 -- C
  assumptions string,                 -- A
  source_span_id string
);

CREATE TAG IF NOT EXISTS Motivation(
  vid string, text string, source_span_id string
);

CREATE TAG IF NOT EXISTS Gap(
  vid string, text string, source_span_id string
);

CREATE TAG IF NOT EXISTS Contribution(
  vid string, text string, contribution_type string,  -- theoretical / empirical / methodological
  source_span_id string
);

CREATE TAG IF NOT EXISTS Hypothesis(
  vid string, text string, source_span_id string
);

CREATE TAG IF NOT EXISTS Finding(
  vid string,
  finding_type string,                -- quantitative / qualitative
  effect_size float DEFAULT 0.0,
  metric_name string, metric_value string,
  source_span_id string
);

CREATE TAG IF NOT EXISTS Mechanism(
  vid string, text string, source_span_id string
);

CREATE TAG IF NOT EXISTS Limitation(
  vid string, text string, source_span_id string
);

CREATE TAG IF NOT EXISTS FutureWork(
  vid string, text string, source_span_id string
);
```

### Module D — Citation Relationships (context, not flat edges)

```ngql
-- Citation is a NODE, not just an edge — carries context
CREATE TAG IF NOT EXISTS CitationContext(
  vid string,
  cite_type string,                   -- strong / weak / direct / indirect
  relation string,                    -- support / contrast / extend / background
  evidence_section string,            -- section index in citing paper
  evidence_paragraph string,          -- paragraph index
  temporal_signal string DEFAULT '',  -- "prior art" / "concurrent" / "follow-up"
  source_span_id string
);
```

### Module E — Knowledge Relations (25 types, curated to 18)

Full Agents-K1 taxonomy: 25 types in 5 groups. We adopt **18** (skip domain-specific causal ones that need GPU-trained models to detect reliably):

**Controlled (6 — deterministic, GLiNER/header-detectable):**
```ngql
CREATE EDGE BUILDS_ON();
CREATE EDGE USES_COMPONENT();
CREATE EDGE ALTERNATIVE_TO();
CREATE EDGE SOLVES();
CREATE EDGE APPLIED_TO();           -- Method → Task (existing)
CREATE EDGE TARGETS();
```

**Composition (5 — structural):**
```ngql
CREATE EDGE USES_TECHNIQUE();
CREATE EDGE CONSISTS_OF();
CREATE EDGE IMPLEMENTS();
CREATE EDGE COMBINES();
CREATE EDGE REQUIRES();
```

**Methodological comparison (4 — upgrade-mode LLM):**
```ngql
CREATE EDGE DERIVED_FROM();
CREATE EDGE DIFFERS_FROM();
CREATE EDGE HAS_LIMITATION();
CREATE EDGE ADDRESSES_PROBLEM();
```

**Citation argumentative (3 — from Module D):**
```ngql
CREATE EDGE SUPPORTS();             -- via CitationContext.relation=support
CREATE EDGE CONTRASTS();            -- via CitationContext.relation=contrast
CREATE EDGE EXTENDS();              -- via CitationContext.relation=extend
```

**Skipped (7 — need GRPO/causal models, deferred):**
`MOTIVATED_BY · HAS_PROPERTY · SUBSET_OF · CAUSES · ENABLES · INHIBITS · MODULATES · CORRELATED_WITH`

These require causal reasoning models (GRPO-trained). Deferred until we have a fine-tuned extractor or enough LLM budget for reliable extraction.

---

## 3. Core-then-Modes Extraction Factorization

Replaces ADR-037 §4.2's "statistical-first → LLM residual" with formal Core-then-Modes:

### Core stage (2 LLM passes per chunk — OR 0 if GLiNER covers)

| Pass | What | Method | Cost |
|------|------|--------|------|
| Core-1 | Typed entity extraction | **GLiNER 2 offline NER** (CPU, zero API) | 0 LLM |
| Core-2 | Binary relation skeleton | **header-priority** + proximity (deterministic) | 0 LLM |
| Core-2b | Binary relation (residual) | **LLM** (only if GLiNER RE insufficient) | 1 LLM/chunk |

### Projection modes (0 LLM — deterministic from Core)

| Mode | What | Method |
|------|------|--------|
| Binary projection | Flatten n-ary to binary relations | Graph projection on Core entities |
| Person extraction | Extract author/person entities | Regex + GROBID header |
| Surface canonicalization | Merge surface variants → stable IDs | SHA256 hash join (Module A) |

### Upgrade modes (1 LLM pass per chunk — only when needed)

| Mode | What | When | Method |
|------|------|------|--------|
| N-ary relations | arity ≥3 hyperedges | when Core finds complex sentences | LLM structured extract |
| Temporal | valid_from/valid_to/version | on re-parse or version update | LLM temporal tag |
| Abstract (Module C) | Motivation, Gap, Contribution, Finding | on synthesis request | LLM per section |
| Citation context | Module D: cite_type, relation | on bibliography parse | LLM + GROBID citations |
| DIY | domain-specific custom | on-demand | LLM with custom schema |

### Cost math (Agents-K1 proof)

For `n=8 chunks, n_up=4 upgrade modes, M=6 views`:
- **Naive:** 8 × 6 × 2 = **96 LLM calls**
- **Core-then-Modes:** 8 × (2 + 4) = **48** → **−50%**

With GLiNER replacing Core-1:
- **Our variant:** 0 (GLiNER) + 8 × 1 (Core-2b residual) + 8 × 4 (upgrade) = **40** → **−58%**

---

## 4. Tri-Source Retrieval (extends ADR-037 §4.4 agent flow)

```text
Agent Query
  │
  ├──► S_web (Web Search)
  │    arXiv API, Semantic Scholar, Google Scholar
  │    sim(title, q) · 0.6 + sim(abstract, q) · 0.4
  │    → fresh papers, API-only metadata
  │
  ├──► S_mmkg (Multimodal Graph Retrieval)  ← RuVector + Samyama Graph
  │    hybrid dense (HNSW) + lexical (BM25)
  │    over figures, tables, equations, code
  │    → primary retrieval source
  │
  └──► S_kn (Knowledge Network Traversal)  ← Samyama Graph only
       typed relation traversal — NO vector search
       long-chain causal: A BUILDS_ON B USES_COMPONENT C ...
       → reasoning paths unreachable by embeddings
```

### Fusion weights (configurable per query type)

| Query type | S_web | S_mmkg | S_kn | When |
|-----------|:-----:|:------:|:----:|------|
| **default** | 0.30 | 0.40 | 0.30 | balanced |
| **recency** | 0.70 | 0.15 | 0.15 | "latest papers on X" |
| **multimodal** | 0.15 | 0.70 | 0.15 | "tables comparing X" |
| **causal** | 0.10 | 0.20 | 0.70 | "why does X improve Y" |
| **synthesis** | 0.20 | 0.50 | 0.30 | "survey of methods for X" |

### Rust implementation

```rust
// da-application/src/agent/search.rs

pub struct TriSourceRetriever {
    web: Box<dyn WebSearchPort>,
    mmkg: Box<dyn HybridSearchPort>,     // RuVector HNSW + BM25
    kn: Box<dyn GraphTraversalPort>,      // Samyama Graph typed edges
    fusion: FusionWeights,
}

impl TriSourceRetriever {
    pub fn retrieve(&self, query: &Query, mode: QueryMode) -> Vec<Candidate> {
        let (w_web, w_mmkg, w_kn) = self.fusion.for_mode(mode);
        
        let web_results = self.web.search(&query.text, 50);       // S_web
        let mmkg_results = self.mmkg.hybrid_search(&query, 100);   // S_mmkg
        let kn_results = self.kn.traverse(&query.entities, 3);     // S_kn: 3-hop typed
        
        // Fusion: weighted merge + dedup by stable ID
        self.fuse(web_results, mmkg_results, kn_results, w_web, w_mmkg, w_kn)
            .into_iter()
            .dedup_by(|a, b| a.vid == b.vid)
            .collect()
    }
}
```

---

## 5. Six Graph Operators (Samyama Graph port operations)

```rust
// da-ports/src/graph_store.rs — added to GraphStore trait

pub trait GraphStore {
    // ... existing CRUD ...
    
    /// O1: Seed Resolution — mention strings → canonical node sets
    fn seed_resolve(&self, mentions: &[String]) -> Result<Vec<CanonicalNode>>;
    
    /// O2: Citation Lineage — forward/backward traversal + shortest-path
    fn citation_lineage(&self, paper_vid: &str, direction: Direction, depth: usize) 
        -> Result<CitationLineage>;
    
    /// O3: Comparative Baseline — methods by dataset/metric
    fn comparative_baselines(&self, dataset_vid: &str, metric_vid: &str) 
        -> Result<Vec<BaselineComparison>>;
    
    /// O4: Multimodal Anchor — figures/tables/equations for a paper
    fn multimodal_anchors(&self, paper_vid: &str) -> Result<Vec<MultimodalEvidence>>;
    
    /// O5: Gap Detection — orphan methods, singleton datasets, sparse cells
    fn detect_gaps(&self, domain: &str) -> Result<Vec<KnowledgeGap>>;
    
    /// O6: Idea Grounding / Novelty — overlap by problem formulation
    fn novelty_judge(&self, idea: &Idea) -> Result<NoveltyReport>;
}
```

### Samyama Graph Cypher implementations

```ngql
-- O1: Seed Resolution
MATCH (n) WHERE n.label IN $mentions OR n.canonical_name IN $mentions
RETURN n;

-- O2: Citation Lineage (backward, 3-hop)
GET SUBGRAPH 3 STEPS FROM $paper_vid BOTH CITES, SUPPORTS, CONTRASTS, EXTENDS;

-- O3: Comparative Baselines
MATCH (m:Method)-[:EVALUATED_ON]->(d:Dataset {vid: $dataset_vid})
MATCH (m)-[:APPLIED_TO]->(t:Task)
OPTIONAL MATCH (m)-[:OUTPERFORMS]->(b:Model)
RETURN m, d, t, b;

-- O4: Multimodal Anchor
MATCH (p:Paper {vid: $paper_vid})-[:HAS_FIGURE]->(f:Figure)
MATCH (p)-[:HAS_TABLE]->(t:Table)
MATCH (p)-[:HAS_EQUATION]->(e:Equation)
RETURN f, t, e;

-- O5: Gap Detection
MATCH (m:Method) WHERE NOT (m)-[:EVALUATED_ON]->(:Dataset)
RETURN m AS orphan_method;
MATCH (d:Dataset) WHERE NOT ()-[:EVALUATED_ON]->(d)
RETURN d AS singleton_dataset;

-- O6: Novelty Judge
MATCH (idea:ProblemDefinition)
MATCH (existing:ProblemDefinition)
WHERE idea.input_vars = existing.input_vars
  AND idea.output_vars = existing.output_vars
RETURN existing, count(*) AS overlap;
```

---

## 6. Identifier-Preserving Joins (theoretical foundation)

Adopt Agents-K1 Proposition P1:

> If all views use stable IDs (SHA256 canonical), cross-view join is O(|K|) hash join, no false merges.

**Implementation rule:** Every entity in da-domain MUST have a `vid: String` computed as `SHA256(canonical_form)`. Surface variants merge to the same vid. This enables:
- Cheap cross-source dedup (papers from arXiv vs PubMed vs Semantic Scholar)
- Cheap agent memory recall (trajectory entities → graph entities)
- Cheap tri-source fusion (S_web + S_mmkg + S_kn merge by vid)

```rust
// da-domain/src/vid.rs

pub fn paper_vid(arxiv_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"paper:");
    hasher.update(arxiv_id.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn entity_vid(entity_type: &str, label: &str) -> String {
    let canonical = label.to_lowercase().trim();
    let mut hasher = Sha256::new();
    hasher.update(entity_type.as_bytes());
    hasher.update(b":");
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}
```

---

## 7. Hyperedge Awareness (P2 strict gap)

Agents-K1 P2: binary graphs lose arity-3+ relation endpoints.

**Our approach:** Samyama Graph supports hyperedges natively. We store:
- Binary relations as edges (BUILDS_ON, USES_COMPONENT, etc.)
- N-ary relations as intermediate nodes with typed edges to all participants

```ngql
-- "Method M uses Dataset D on Task T with Metric Mt"
-- is a 4-ary relation → represented as a hyperedge node:
CREATE TAG IF NOT EXISTS ExperimentSetup(
  vid string,
  description string
);

-- Then:
MATCH (m:Method)-[:PART_OF]->(e:ExperimentSetup)<-[:PART_OF]-(d:Dataset)
MATCH (e)<-[:PART_OF]-(t:Task)
MATCH (e)<-[:PART_OF]-(mt:Metric)
```

This avoids the P2 strict gap: no information loss from arity reduction.

---

## 8. Small-Model Extraction Principle

Agents-K1 uses Qwen3-4B + GRPO for extraction, GPT-5.2 for reasoning.

**Our equivalent (no GPU):**

| Extraction task | Agent-K1 | daily-archive v2 | Cost |
|----------------|----------|-------------------|------|
| Entity NER | Qwen3-4B GRPO | **GLiNER 2 offline** (CPU, char spans) | 0 |
| Binary relations | Qwen3-4B GRPO | **header-priority + GLiNER RE** | 0 |
| Residual relations | Qwen3-4B GRPO | **GLM-5.2 via 9router** (rate-limited) | low |
| Module C (abstract) | Qwen3-4B GRPO | **GLM-5.2** (only on synthesis request) | low |
| Module D (citation) | Qwen3-4B GRPO | **GROBID TEI + GLM-5.2 residual** | low |
| Final synthesis | GPT-5.2 | **GLM-5.2 / MiniMax-M3** (SYNTHESIS state only) | yes |

**Key:** extraction uses offline (GLiNER) + small API models. Reasoning uses large models only at SYNTHESIS. Same separation principle as Agents-K1, adapted for CPU-only infrastructure.

---

## 9. Multi-Agent Swarm (refines ADR-037 §4.4)

Agents-K1 defines 6 agent roles. We adopt **4** (skip CodeWiki/Idea for now):

```rust
// da-application/src/agent/swarm.rs

pub enum AgentRole {
    /// Analyzes graph + task → dispatch plan
    Coordinator,
    /// Clusters paper nodes → writes topic sections with citation lineage
    SurveyWorker,
    /// Cross-paper synthesis: compare, contrast, find gaps
    SynthesisWorker,
    /// Merges artifacts → manifest (job ids, status, evidence)
    Aggregator,
}
```

Each role runs its own SymFSM (Planning→Search→Reading→Synthesis→Verify→Learning). The Coordinator dispatches to SurveyWorker/SynthesisWorker, Aggregator collects results.

**Deferred (future):** CodeWikiWorker (code repo documentation), IdeaWorker (novel idea generation), PrototypeWorker (experiment scaffolding).

---

## 10. What we do NOT adopt from Agents-K1

| Agents-K1 feature | Why not | Our alternative |
|---|---|---|
| **GRPO training** | No GPU (ADR-023) | GLiNER 2 + LLM residual |
| **Qwen3-4B fine-tuned** | No training infra | GLiNER 2 + GLM-5.2 |
| **Neo4j** | Scale ceiling (single-node) | Samyama Graph (distributed) |
| **17 MCP servers** | Overkill for MVP | Direct Rust API + single MCP later |
| **CodeWikiWorker** | Not in scope yet | Deferred to Phase 6 |
| **IdeaWorker + PrototypeWorker** | Research novelty generation | Deferred |
| **Causal relations (7 types)** | Need GRPO/causal models | Deferred; 18 types cover 90% |

---

## 11. Updated Phasing

ADR-037 phases, refined with Agents-K1 patterns:

| Phase | Focus | Agents-K1 elements |
|-------|-------|--------------------|
| **1. Foundation** | da-domain + ports + Samyama Graph adapter | Module A schema, stable VIDs |
| **2. Ingest** | GROBID + ODL + preprocess | MinerU = ODL (confirmed) |
| **3. Extraction** | GLiNER 2 + Core-then-Modes | Core (GLiNER) + Projection (0 LLM) + Upgrade (LLM residual) |
| **4. Schema** | Module B-E in Samyama Graph | 18 relation types, multimodal nodes, hyperedge ExperimentSetup |
| **5. Search** | Tri-source + 6 operators | S_web + S_mmkg + S_kn, O1-O6 |
| **6. Agent** | SymFSM + multi-role swarm | Coordinator + Survey + Synthesis + Aggregator |
| **7. Scale** | 1M → 10M → 100M | DiskANN + Samyama Graph distributed |

---

## 12. Experience from Python M001-M284 applied

| Python lesson | Agents-K1 alignment | ADR-038 application |
|---|---|---|
| Evidence chain must be immutable | Module D citation context | CitationContext node with source_span_id |
| Layout spans are page/bbox | O4 multimodal anchors | Figure/Table/Equation with page+bbox |
| Statistical-first reduces cost | Core-then-Modes −50% | GLiNER Core + deterministic Projection |
| Stable IDs enable cheap joins | P1 identifier-preserving | SHA256 vid for every entity |
| Hyperedges avoid info loss | P2 strict gap | ExperimentSetup n-ary nodes |
| Small extract, large reason | 4B extract + GPT reason | GLiNER extract + GLM/MiniMax reason |
| Citation is context, not edge | Module D | CitationContext as NODE, not edge |
