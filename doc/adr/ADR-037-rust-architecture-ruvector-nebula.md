# ADR-037: Rust Architecture — Daily-Archive v2 (RuVector + Samyama Graph)

**Status:** Accepted (binding) — partially superseded by ADR-040 (graph choice: NebulaGraph → Samyama Graph; schema: nGQL → Cypher) and ADR-041 (embedded Cypher + HOT path). Architecture layers (§2), parsers (§4.1), and RuVector agent brain remain binding.
**Date:** 2026-07-25
**Deciders:** human
**Replaces in Rust project:** ADR-023 (Python 7-layer), ADR-022 (FalkorDB), ADR-016 (NetworkX intermediate). Python ADRs remain valid for the frozen `legacy/` codebase.

> **ADR-040 supersedes the graph store choice in this ADR:** NebulaGraph → **Samyama Graph** (Rust-native, proven 74M nodes). RuVector narrows to agent brain only (~10 crates: see ADR-040 §3). RVF = agent experience container. See ADR-040 for the locked technology stack.
**Binding Level:** binding
**Revisable:** yes, with milestone evidence

---

## 0. One-line Decision

> daily-archive will be rebuilt in **Rust** as a **hexagonal (ports-and-adapters) system** with 6 concentric layers: Domain → Ports → Application → Adapters → Infrastructure → Composition. The agent layer uses **SymFSM-controlled state machines** + **RuVector** primitives (SONA, GNN-rerank, agent memory, hybrid BM25+RRF, PPR) to minimize LLM cost. The knowledge graph persists in **Samyama Graph** (distributed, 1M–100M scale). Versioning and temporality are first-class: every entity carries `(valid_from, valid_to, superseded_by)`.

## 1. Vision (Consensus.app++)

We build an **AI-native scientific knowledge engine** inspired by [consensus.app](https://consensus.app) but with higher ambitions:

| Consensus.app does | daily-archive v2 will do (beyond) |
|---|---|
| Search 220M papers (BM25 + semantic) | Search 1M–100M papers **with evidence-traced spans** (page/bbox/char) |
| 3-step ranking (relevance → quality → precision) | 3-step + **GNN-rerank** (graph structure) + **PPR influence** + **SONA self-learning** |
| Multi-agent Scholar (Planning → Search → Reading → Analysis) | SymFSM-controlled agents with **deterministic state machines**, **agent memory traces**, **experience store** |
| Citation-backed answers | Citation-backed answers **with immutable evidence chain** (PDF hash + TEI + ODL layout + page/bbox) |
| "Checker models" verify relevance | **Fail-closed promotion gate** — no graph write without full evidence chain + explicit human go |
| Proprietary corpus + partnerships | Open-source, **self-hosted**, any corpus (arXiv, PubMed, textbooks, code, datasets) |
| No temporality | **Full temporality**: track paper versions, edition history, supersession chains |
| No local LLM | Optional **ruvllm local LLM** for offline/edge deployment (zero API cost) |
| No agent learning | **SONA self-learning**: retrieval improves from reward trajectories |

### Core principle (from Consensus)

> "We only use AI **after** we search the scientific literature. Every response is grounded in real, citable research."

daily-archive v2 adds: **every citation is traceable to an immutable source artifact with page/bbox coordinates.**

---

## 2. Hexagonal Architecture (Onion)

```text
┌─────────────────────────────────────────────────────────┐
│                    Composition Root                      │
│              (CLI / Server / MCP / WASM)                 │
├─────────────────────────────────────────────────────────┤
│  Infrastructure                                          │
│  ┌─────────────┐ ┌──────────┐ ┌───────────────────────┐ │
│  │ Samyama Graph │ │ RuVector │ │ GROBID/ODL/GLiNER     │ │
│  │ Adapter     │ │ Embedded │ │ HTTP/Subprocess       │ │
│  └─────────────┘ └──────────┘ └───────────────────────┘ │
├─────────────────────────────────────────────────────────┤
│  Adapters (implement Ports)                              │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────┐ │
│  │Samyama Graph│ │RVF Store │ │GROBID    │ │LLM Client   │ │
│  │GraphStore │ │VectorStore│ │Parser    │ │(9router/ruv)│ │
│  └──────────┘ └──────────┘ └──────────┘ └────────────┘ │
├─────────────────────────────────────────────────────────┤
│  Application (use cases / orchestrators)                 │
│  ┌────────┐ ┌──────────┐ ┌────────┐ ┌────────────────┐ │
│  │ Ingest │ │ Preprocess│ │ Extract│ │ AgentOrchestra │ │
│  │Pipeline│ │ Stack     │ │Pipeline│ │ (SymFSM+RuVec) │ │
│  └────────┘ └──────────┘ └────────┘ └────────────────┘ │
│  ┌────────────────┐ ┌────────────┐ ┌────────────────┐ │
│  │ ETL Scheduler  │ │ Review Gate│ │ Graph Writer   │ │
│  │ (resource-aware)│ │(fail-closed)│ │(Samyama+version)│ │
│  └────────────────┘ └────────────┘ └────────────────┘ │
├─────────────────────────────────────────────────────────┤
│  Ports (traits / interfaces)                             │
│  GraphStore · VectorStore · Embedder · ParserPort        │
│  LLMClient · SchedulerPort · EvidenceStore               │
├─────────────────────────────────────────────────────────┤
│  Domain (pure types + rules, zero deps)                  │
│  Paper · Author · Citation · Entity · Relation           │
│  EvidenceAssertion · SourceSpan · CanonicalDocument      │
│  AgentState · Trajectory · RewardSignal                  │
│  Versioned<T> · TemporalRecord                           │
└─────────────────────────────────────────────────────────┘
```

**Layer rules (enforced by onion guard):**

| Layer | Depends on | Examples | Never depends on |
|-------|-----------|----------|-----------------|
| **Domain** | nothing (only std + serde) | types, enums, validation rules | tokio, reqwest, any adapter |
| **Ports** | Domain | `trait GraphStore`, `trait Embedder` | concrete implementations |
| **Application** | Domain + Ports | `IngestUseCase`, `AgentOrchestrator` | reqwest, samyama_client |
| **Adapters** | Ports + infra | `Samyama GraphStore`, `SamyamaVectorStore` | domain types changed |
| **Infrastructure** | external | Samyama Graph, RuVector, GROBID | our domain logic |
| **Composition** | everything | `main.rs`, DI wiring, config | nothing depends on it |

---

## 3. Crate Structure

```text
daily-archive/
├── Cargo.toml                    # workspace
├── crates/
│   ├── da-domain/                # Layer 1: pure domain types + rules
│   │   ├── entity.rs             # Paper, Author, Citation, Entity, Relation
│   │   ├── evidence.rs           # EvidenceAssertion, SourceSpan, CanonicalDocument
│   │   ├── agent.rs              # AgentState, Trajectory, RewardSignal, FSM states
│   │   ├── versioning.rs         # Versioned<T>, TemporalRecord, Supersession
│   │   ├── domain_profile.rs     # Paper / Textbook / CodeRepo / Dataset / TechDoc
│   │   └── rules.rs              # validation rules, type constraints
│   │
│   ├── da-ports/                 # Layer 2: trait definitions
│   │   ├── graph_store.rs        # trait GraphStore (create/query/traverse)
│   │   ├── vector_store.rs       # trait VectorStore (insert/search/hybrid)
│   │   ├── embedder.rs           # trait Embedder (embed text → Vec<f32>)
│   │   ├── parser.rs             # trait Parser (parse raw → ParsedArticle)
│   │   ├── llm_client.rs         # trait LLMClient (chat/extract/structured)
│   │   ├── scheduler.rs          # trait Scheduler (enqueue/dequeue/priority)
│   │   ├── evidence_store.rs     # trait EvidenceStore (persist/resolve spans)
│   │   └── agent_memory.rs       # trait AgentMemory (store/replay/compact)
│   │
│   ├── da-application/           # Layer 3: use cases + orchestrators
│   │   ├── ingest/               # source registration, fetch, dedup, catalog
│   │   ├── preprocess/           # body clean, quality, outline, keywords (non-LLM)
│   │   ├── extract/              # entity/relation extraction (statistical-first)
│   │   ├── review/               # fail-closed gates, promotion boundary
│   │   ├── graph_write/          # Samyama Graph writes with versioning + temporality
│   │   ├── etl_scheduler/        # resource-aware queue, rate limits, priority
│   │   └── agent/                # SymFSM orchestrator, SONA integration
│   │       ├── fsm/              # state machine definitions
│   │       ├── planning.rs       # query decomposition
│   │       ├── search.rs         # hybrid search + GNN-rerank
│   │       ├── reading.rs        # paper analysis + evidence extraction
│   │       ├── synthesis.rs      # cross-paper synthesis + answer
│   │       └── experience.rs     # trajectory storage + SONA reward
│   │
│   ├── da-adapters/              # Layer 4: port implementations
│   │   ├── samyama_graph.rs       # GraphStore impl via Samyama Graph Rust client
│   │   ├── ruvector_store.rs     # VectorStore impl via RVF/HNSW
│   │   ├── onnx_embedder.rs      # Embedder impl via bge-m3 ONNX
│   │   ├── grobid_parser.rs      # Parser impl via GROBID HTTP
│   │   ├── odl_parser.rs         # Parser impl via OpenDataLoader subprocess
│   │   ├── gliner2_extractor.rs  # entity extraction via GLiNER 2 (PyO3/subprocess)
│   │   ├── ninerouter_llm.rs     # LLMClient impl via 9router OpenAI-compatible
│   │   ├── ruvllm_local.rs       # LLMClient impl via ruvllm (local, zero-cost)
│   │   └── ruvector_memory.rs    # AgentMemory impl via ruvector-agent-memory
│   │
│   ├── da-graph/                 # knowledge graph operations on Samyama Graph
│   │   ├── schema.rs             # spaces, tags, edges (Paper, Author, CITES, ...)
│   │   ├── queries.rs            # Cypher query builders
│   │   ├── pagerank.rs           # PPR / PageRank via Samyama Graph algorithms
│   │   ├── community.rs          # Louvain / community detection
│   │   ├── subgraph.rs           # k-hop subgraph extraction
│   │   └── temporal.rs           # versioned queries (as-of, history)
│   │
│   ├── da-vector/                # vector + hybrid search on RuVector
│   │   ├── hnsw_cache.rs         # local HNSW cache (working set)
│   │   ├── hybrid_search.rs      # BM25 + RRF fusion
│   │   ├── gnn_rerank.rs         # GNN reranking on subgraphs
│   │   └── diskann.rs            # DiskANN for SSD-backed large scale
│   │
│   ├── da-agent-brain/           # RuVector-backed agent intelligence
│   │   ├── sona.rs               # SONA self-learning integration
│   │   ├── memory.rs             # agent memory compaction + retention
│   │   ├── trajectory.rs         # trajectory buffer + reasoning bank
│   │   └── mincut.rs             # MinCut for experience graph partitioning
│   │
│   ├── da-cli/                   # CLI binary
│   ├── da-server/                # HTTP/gRPC server binary (future)
│   └── da-mcp/                   # MCP server binary (future)
│
├── legacy/                       # frozen Python codebase (reference only)
├── data/                         # canonical catalog, PDFs (gitignored)
├── artifacts/                    # ETL artifacts, reports
└── doc/                          # ADRs, architecture docs
```

---

## 4. Data Flow

### 4.1 Ingest Pipeline (new data import)

```text
Source (arXiv/PubMed/HTML/PDF/Code)
  │
  ▼
┌──────────────┐
│  Ingest Gate │  SHA256 dedup → catalog register → domain profile assignment
└──────┬───────┘
       │ paper_id, source_path, profile
       ▼
┌──────────────┐
│   Fetch      │  download PDF / clone repo / fetch HTML (bounded, cached)
└──────┬───────┘
       │ local_path
       ▼
┌──────────────┐
│   Parse      │  GROBID (TEI) + ODL (layout JSON) → ParsedArticle
└──────┬───────┘  persist: PDF hash, TEI, layout JSON, ParserRun
       │ ParsedArticle { sections, blocks, citations, layout }
       ▼
┌──────────────┐
│  Preprocess  │  body clean → quality → language → outline → fingerprint
│  (non-LLM)   │  → keyword spans → term-dense windows
└──────┬───────┘
       │ ArticlePreprocessPackage
       ▼
┌──────────────┐
│  Structure   │  PageIndex tree → SemanticChunks → KnowledgeCard
└──────┬───────┘
       │ structured_document
       ▼
┌──────────────┐
│  Embed       │  OnnxEmbedder (bge-m3) → vectors → RVF store + Samyama Graph
└──────┬───────┘
       │ vector_id, paper_id
       ▼
┌──────────────┐
│  Catalog     │  register in Samyama Graph: Paper node + Author nodes + CITES edges
│  (Samyama)    │  with temporal versioning (valid_from, version)
└──────────────┘
```

### 4.2 Extraction Pipeline (statistical-first → LLM residual)

```text
StructuredDocument
  │
  ├──► Statistical (0 LLM cost)
  │    ├── YAKE keywords (via ruvector hybrid BM25)
  │    ├── TF-IDF extractive summary
  │    ├── GLiNER 2 offline NER (CPU, no API)
  │    ├── Header-priority candidate selection
  │    └── Graph community pre-clustering
  │
  ├──► Statistical output: candidates, keywords, communities
  │
  ▼
┌──────────────┐
│ LLM Residual │  only when statistical insufficient:
│ (cost-aware) │  typed entity classification, relation typing, abstract extraction
│              │  rate-limit checked, provider fallback
└──────┬───────┘
       │ typed entities + relations
       ▼
┌──────────────┐
│ Evidence     │  attach SourceSpan (page/bbox/char) to every extraction
│ Grounding    │  resolve spans against layout JSON
└──────┬───────┘
       │ EvidenceAssertion[]
       ▼
┌──────────────┐
│ Review Gate  │  fail-closed: faithfulness check, precision gate
│              │  high-impact relations need extra verification
└──────┬───────┘  (blocked → repair; passed → staging)
       │ staged evidence
       ▼
┌──────────────┐
│ Graph Write  │  Samyama Graph: Entity/Relation nodes + edges
│ (versioned)  │  with EvidenceAssertion linkage + temporal versioning
└──────────────┘  import_eligible: false until explicit human go
```

### 4.3 ETL Scheduler (resource-aware)

```text
┌─────────────────────────────────────────────┐
│            ETL Scheduler (tokio)             │
│                                              │
│  Priority Queue:                             │
│  ┌─────────┐ ingest(NEW)     priority: HIGH  │
│  │         │ reparse(FAIL)   priority: HIGH  │
│  │         │ extract(PEND)   priority: MED   │
│  │         │ embed(STALE)    priority: LOW   │
│  │         │ graph_write(OK) priority: LOW   │
│  └─────────┘                                 │
│                                              │
│  Resource Guards:                            │
│  ├── CPU budget (concurrent tasks ≤ N)       │
│  ├── RAM budget (HNSW cache size)            │
│  ├── LLM rate limits (per-provider)          │
│  ├── GROBID/ODL sidecar health               │
│  └── Samyama Graph write throughput            │
│                                              │
│  Backpressure:                               │
│  ├── if GROBID slow → batch ODL only         │
│  ├── if LLM rate-limited → statistical-only  │
│  ├── if Samyama saturated → defer graph_write │
│  └── if disk < threshold → pause ingest      │
│                                              │
│  Checkpointing:                              │
│  ├── WAL per task (crash recovery)           │
│  ├── idempotent (SHA256 dedup)               │
│  └── resume from last checkpoint             │
└─────────────────────────────────────────────┘
```

### 4.4 Agent Flow (SymFSM-controlled, Consensus-inspired)

```text
User Query: "What methods improve link prediction on knowledge graphs?"
  │
  ▼
┌──────────────────────────────────────────────────────┐
│  FSM State: PLANNING                                  │
│  ├── decompose query into sub-questions               │
│  ├── classify query type (factual/synthesis/exploration)│
│  └── decide tool sequence                             │
│  transition: → SEARCHING                              │
├──────────────────────────────────────────────────────┤
│  FSM State: SEARCHING                                 │
│  ├── RuVector hybrid search (BM25 + dense + RRF)      │
│  ├── candidate IDs → Samyama Graph k-hop subgraph pull  │
│  ├── GNN-rerank on subgraph (graph-aware)             │
│  ├── PPR influence scores from key papers             │
│  └── top-K papers with evidence spans                 │
│  transition: → READING (if papers found)              │
│             → REPAIR (if insufficient)                 │
├──────────────────────────────────────────────────────┤
│  FSM State: READING                                   │
│  ├── for each paper: extract key findings (GLiNER/LLM) │
│  ├── verify evidence spans (page/bbox grounded)        │
│  ├── check-paper relevance (checker model)            │
│  └── build structured findings per paper              │
│  transition: → SYNTHESIS                              │
├──────────────────────────────────────────────────────┤
│  FSM State: SYNTHESIS                                 │
│  ├── cross-paper synthesis (compare, contrast, group) │
│  ├── conflict detection (contradictory findings)      │
│  ├── generate answer with citation pack               │
│  └── format: summary table + evidence links           │
│  transition: → VERIFY                                 │
├──────────────────────────────────────────────────────┤
│  FSM State: VERIFY                                    │
│  ├── structural verifier: every claim → grounded span │
│  ├── citation checker: every cite → real paper        │
│  ├── if gaps → REPAIR                                 │
│  └── if pass → OUTPUT                                 │
├──────────────────────────────────────────────────────┤
│  FSM State: REPAIR (can loop back max 2×)             │
│  ├── clarify query (ask user or broaden)              │
│  ├── decompose into smaller sub-queries               │
│  ├── reframe (different search strategy)              │
│  └── transition: → SEARCHING / → OUTPUT(honest gap)   │
├──────────────────────────────────────────────────────┤
│  FSM State: OUTPUT                                    │
│  ├── deliver citation-backed answer                   │
│  ├── research context pack (papers + metadata + findings)│
│  ├── evidence trace (every span → PDF page/bbox)      │
│  └── transition: → LEARNING                           │
├──────────────────────────────────────────────────────┤
│  FSM State: LEARNING                                  │
│  ├── user feedback (implicit/explicit reward)         │
│  ├── trajectory → SONA (update retrieval weights)     │
│  ├── trajectory → experience store (case-based)        │
│  └── transition: → IDLE                               │
└──────────────────────────────────────────────────────┘
```

**LLM cost reduction via RuVector:**

| Stage | LLM cost | RuVector alternative |
|-------|---------|---------------------|
| Search | 0 | hybrid search (BM25+RRF+HNSW) |
| Candidate rerank | 0 | GNN-rerank on subgraph |
| Paper reading (entities) | low | GLiNER 2 offline NER (CPU) |
| Paper reading (relations) | low | GLiNER 2 RE / header-priority |
| Influence ranking | 0 | PPR ForwardPush |
| Community context | 0 | Louvain / MinCut |
| Memory compaction | 0 | ruvector-agent-memory (LRU/LFU/coherence) |
| Retrieval improvement | 0 | SONA self-learning |
| Final synthesis | **yes** | LLM fills structured template only |

**Result:** LLM is invoked only at SYNTHESIS (and optionally READING for complex papers). Everything else is offline/algorithmic.

---

## 5. Knowledge Graph Schema (Samyama Graph)

> **See ADR-038 for the authoritative expanded schema** (5 modules A-E, ~20 node types, 18 relation types, CitationContext-as-node, multimodal nodes). The schema below is the **initial subset** from ADR-037; ADR-038 §2 supersedes it with the full Agents-K1-aligned schema. In case of conflict, ADR-038 + ADR-039 lifecycle tags prevail.

### 5.1 Initial Tags (subset — expanded by ADR-038)

```ngql
-- Space with 1 partition (dev) → scale partitions for prod
CREATE SPACE IF NOT EXISTS daily_archive(
  partition_num = 100,
  replica_factor = 1,
  vid_type = FIXED_STRING(64)
);

USE daily_archive;

-- Tags (node types)
CREATE TAG IF NOT EXISTS Paper(
  arxiv_id string,
  title string,
  abstract string,
  doi string,
  pdf_hash string,
  published_at timestamp,
  ingested_at timestamp,
  domain_profile string DEFAULT 'paper',
  -- versioning + temporality
  valid_from timestamp,
  valid_to timestamp DEFAULT 0,        -- 0 = current
  version int DEFAULT 1,
  superseded_by string DEFAULT '',
  -- quality signals
  citation_count int DEFAULT 0,
  journal_impact float DEFAULT 0.0,
  -- evidence
  evidence_ready bool DEFAULT false,
  import_eligible bool DEFAULT false
);

CREATE TAG IF NOT EXISTS Author(
  name string,
  canonical_name string,
  orcid string DEFAULT '',
  institution string DEFAULT '',
  valid_from timestamp,
  valid_to timestamp DEFAULT 0,
  version int DEFAULT 1
);

CREATE TAG IF NOT EXISTS Entity(
  label string,
  entity_type string,    -- Method, Dataset, Model, Task, Metric, Field
  description string DEFAULT '',
  source_span_id string, -- links to EvidenceAssertion
  confidence float DEFAULT 0.0,
  valid_from timestamp,
  valid_to timestamp DEFAULT 0,
  version int DEFAULT 1
);

CREATE TAG IF NOT EXISTS Topic(
  name string,
  description string DEFAULT '',
  parent_topic string DEFAULT ''
);

CREATE TAG IF NOT EXISTS Institution(
  name string,
  country string DEFAULT '',
  ror_id string DEFAULT ''
);

CREATE TAG IF NOT EXISTS EvidenceAssertion(
  claim string,
  span_type string,       -- page_bbox, char_only, tei
  page int,
  bbox string,            -- JSON [x1,y1,x2,y2]
  char_start int,
  char_end int,
  artifact_hash string,   -- PDF/TEI/ODL hash
  epistemic_status string, -- verified, staged, pending
  created_at timestamp
);
```

### 5.2 Edge Types

```ngql
CREATE EDGE IF NOT EXISTS CITES(
  context string DEFAULT '',
  in_bibliography bool DEFAULT false
);
-- NOTE: ADR-038 Module D supersedes flat CITES with CitationContext-as-NODE
-- carrying cite_type, relation (support/contrast/extend), evidence location.
-- The CITES edge above is retained for simple lineage; CitationContext is
-- the authoritative citation model per ADR-038.

CREATE EDGE IF NOT EXISTS AUTHORED_BY(
  position int DEFAULT 0,
  corresponding bool DEFAULT false
);

CREATE EDGE IF NOT EXISTS AFFILIATED_WITH();

CREATE EDGE IF NOT EXISTS BELONGS_TO_TOPIC(
  weight float DEFAULT 1.0
);

CREATE EDGE IF NOT EXISTS APPLIED_TO();      -- Method → Task
CREATE EDGE IF NOT EXISTS USES_COMPONENT();  -- Model → Method
CREATE EDGE IF NOT EXISTS EVALUATED_ON();    -- Method → Dataset
CREATE EDGE IF NOT EXISTS OUTPERFORMS(       -- Model → Model
  metric string,
  delta float
);

CREATE EDGE IF NOT EXISTS HAS_EVIDENCE();    -- Entity → EvidenceAssertion
CREATE EDGE IF NOT EXISTS SUPERSEDES(        -- versioning
  reason string,
  at timestamp
);
```

### 5.3 Vector Index (Samyama Graph or external)

Samyama Graph v3.8+ supports vector indexes natively. If unavailable, use RuVector DiskANN for vector storage with ID sync to Samyama Graph.

```ngql
-- Samyama Graph native vector (if supported)
CREATE TAG INDEX IF NOT EXISTS paper_embedding ON Paper(embedding(1024));
```

---

## 6. Versioning + Temporality

Every mutable entity is wrapped in `Versioned<T>`:

```rust
// da-domain/src/versioning.rs

pub struct Versioned<T> {
    pub current: T,
    pub valid_from: chrono::DateTime<chrono::Utc>,
    pub valid_to: Option<chrono::DateTime<chrono::Utc>>,
    pub version: u32,
    pub superseded_by: Option<String>,  // VID of newer version
}

pub struct TemporalRecord<T> {
    pub entity_id: String,
    pub history: Vec<Versioned<T>>,  // ordered by valid_from
}

impl<T> TemporalRecord<T> {
    /// Get the version effective at a point in time (bi-temporal).
    pub fn as_of(&self, when: chrono::DateTime<chrono::Utc>) -> Option<&Versioned<T>> {
        self.history.iter()
            .rev()
            .find(|v| v.valid_from <= when && v.valid_to.map_or(true, |to| to > when))
    }

    pub fn current(&self) -> Option<&Versioned<T>> {
        self.history.iter().rev().find(|v| v.valid_to.is_none())
    }

    pub fn supersede(&mut self, new_version: T, reason: &str) {
        if let Some(last) = self.history.last_mut() {
            last.valid_to = Some(chrono::Utc::now());
        }
        self.history.push(Versioned {
            current: new_version,
            valid_from: chrono::Utc::now(),
            valid_to: None,
            version: self.history.len() as u32 + 1,
            superseded_by: None,
        });
    }
}
```

**Use cases:**
- Paper v1 (preprint) → v2 (camera-ready): `SUPERSEDES` edge, both retained
- Author name change: old name `valid_to` set, new `valid_from` set
- Entity merge: duplicate entities linked via `SUPERSEDES(reason="merged_into")`
- Citation correction: old CITES edge `valid_to`, new CITES edge created

---

## 7. RuVector Integration (Agent Brain)

### 7.1 Role separation

```text
┌────────────────────────────────────┐
│  RuVector (embedded, per-agent)     │
│                                    │
│  ┌──────────────┐  ┌────────────┐  │
│  │ HNSW Cache   │  │ BM25 Index │  │
│  │ (working set)│  │ (keywords) │  │
│  └──────────────┘  └────────────┘  │
│         RRF Fusion → candidates    │
│              ↓                     │
│  ┌──────────────┐                  │
│  │ GNN Rerank   │ ← subgraph from  │
│  │              │   Samyama Graph    │
│  └──────────────┘                  │
│              ↓                     │
│  ┌──────────────┐  ┌────────────┐  │
│  │ Agent Memory │  │ SONA       │  │
│  │ (compaction) │  │ (learning) │  │
│  └──────────────┘  └────────────┘  │
└────────────────────────────────────┘
         ↕ candidate IDs, subgraphs
┌────────────────────────────────────┐
│  Samyama Graph (distributed)         │
│  Full knowledge graph (1M-100M)    │
└────────────────────────────────────┘
```

### 7.2 SONA reward loop

```rust
// da-agent-brain/src/sona.rs

pub struct SonaIntegration {
    trajectory_buffer: TrajectoryBuffer,
    reward_signal: RewardSignal,
    // MicroLoRA adapters that adjust retrieval weights
    lora_adapters: Vec<MicroLora>,
}

impl SonaIntegration {
    /// Called after each agent query completes.
    pub fn record_outcome(&mut self, trajectory: &Trajectory, reward: f32) {
        // 1. Store trajectory in buffer
        self.trajectory_buffer.push(trajectory.clone(), reward);
        
        // 2. Update MicroLoRA weights (EWC-protected)
        self.lora_adapters.iter_mut().for_each(|a| {
            a.update(&trajectory, reward, &self.reward_signal.ewc_constraints);
        });
        
        // 3. Next search will use updated weights → better results
    }
    
    /// Apply learned weights to candidate scoring.
    pub fn adjust_scores(&self, candidates: &mut [Candidate]) {
        for adapter in &self.lora_adapters {
            adapter.apply(candidates);
        }
    }
}
```

### 7.3 Agent memory (experience store)

```rust
// da-agent-brain/src/memory.rs

pub struct AgentMemory {
    store: ruvector_agent_memory::MemoryStore,
}

impl AgentMemory {
    /// Store a successful reasoning pattern for future reuse.
    pub fn store_experience(&mut self, pattern: ReasoningPattern) {
        let embedding = self.embed(&pattern.summary);
        self.store.insert(MemoryEntry {
            id: pattern.id,
            embedding,
            content: pattern.serialize(),
            recency: chrono::Utc::now(),
            frequency: 1,
            coherence_score: pattern.coherence,
        });
    }
    
    /// Recall similar past experiences.
    pub fn recall(&self, query_embedding: &[f32], k: usize) -> Vec<MemoryEntry> {
        self.store.search(query_embedding, k)
    }
    
    /// Compact: LRU + LFU + coherence-weighted retention.
    pub fn compact(&mut self, max_entries: usize) {
        self.store.compact(max_entries);
    }
}
```

---

## 8. Domain Profiles (ADR-032 preserved)

| Profile | Parser | Entity focus | Vector source |
|---------|--------|-------------|---------------|
| **paper** | GROBID + ODL | Method, Dataset, Model, Task, Metric | abstract + body |
| **textbook** | HTML parser | Concept, Definition, Example, Exercise | chapter text |
| **code_repo** | Git + AST | API, Module, Configuration, TestCase | README + docstrings |
| **dataset** | JSON/YAML parser | DatasetSchema, Metric, License | description + card |
| **tech_doc** | Markdown parser | API, Configuration, Concept | section text |

All profiles flow through the same hexagonal layers with profile-specific adapters.

---

## 9. Safety Boundaries (preserved from M274-M284)

1. **Fail-closed import**: no Samyama Graph writes without full evidence chain + explicit human go
2. **Evidence-first**: every entity/relation must have a resolvable SourceSpan
3. **Statistical-first**: deterministic preprocessing before every LLM call
4. **Rate-limit-aware**: per-provider quota checking before API calls
5. **FSM-controlled agents**: no free-form LLM actions; every agent step is a typed state transition
6. **Versioning immutable**: old versions never deleted; `SUPERSEDES` edges track history
7. **Import gate**: `import_eligible = false` until evidence chain green + human yes

---

## 10. Technology Stack

| Component | Technology | Why |
|-----------|-----------|-----|
| Language | Rust | performance, type safety, single binary |
| Graph DB | Samyama Graph | distributed, 1M-100M scale, Cypher, built-in algorithms |
| Vector/Agent | RuVector (embedded) | SONA, GNN-rerank, agent memory, hybrid search, PPR |
| Parser (PDF) | GROBID (Java HTTP) | scholarly structure, TEI, citations |
| Parser (layout) | OpenDataLoader (Python subprocess) | page/bbox layout JSON |
| NER (offline) | GLiNER 2 (Python subprocess/PyO3) | zero-shot entities + relations, char spans |
| Embeddings | bge-m3 via ONNX Runtime | multilingual, 1024d, local |
| LLM (primary) | GLM-5.2 / MiniMax via 9router | multi-provider, rate-limit-aware |
| LLM (local) | ruvllm | offline/edge, zero API cost |
| Persistence (graph) | Samyama Graph | distributed, sharded |
| Persistence (vectors) | RVF / DiskANN | SSD-backed, persistent |
| Persistence (agent) | redb (via RuVector) | embedded, per-agent |
| Scheduler | tokio + priority queue | async, resource-aware |
| Serialization | serde + serde_json | standard Rust |
| CLI | clap | standard Rust |
| Server (future) | axum | async HTTP/gRPC |
| MCP (future) | rmcp | MCP protocol |

---

## 11. Migration Path

### Phase 1: Foundation (domain + ports + basic adapters)
- da-domain: port all domain types from Python (Paper, Entity, EvidenceAssertion, SourceSpan)
- da-ports: define all trait interfaces
- da-adapters: Samyama Graph client, RuVector store, OnnxEmbedder
- Deploy Samyama Graph locally (docker compose)

### Phase 2: Ingest pipeline
- GROBID + ODL parsers (HTTP/subprocess adapters)
- Preprocess stack (non-LLM: clean, quality, outline, keywords)
- Canonical catalog on Samyama Graph
- Ingest 60 canary papers end-to-end

### Phase 3: Extraction + evidence
- GLiNER 2 adapter for offline NER
- Statistical-first extraction (header-priority, YAKE-equivalent)
- LLM residual extraction (rate-limit-aware)
- Evidence grounding (layout span upgrade → page/bbox)
- Review gate (fail-closed)

### Phase 4: Agent + search
- RuVector hybrid search (BM25 + HNSW + RRF)
- GNN-rerank on Samyama Graph subgraphs
- PPR influence ranking
- SymFSM agent orchestrator (Planning → Search → Reading → Synthesis → Verify)
- SONA reward loop
- Agent memory + experience store

### Phase 5: Scale
- Samyama Graph production cluster (3+ nodes)
- DiskANN for vector scale
- Multi-agent concurrent access (Samyama Graph distributed)
- 1M → 10M → 100M papers

---

## 12. Lessons from Python (M001–M284) applied

| Lesson | Applied as |
|--------|-----------|
| Evidence chain must be immutable (PDF hash + TEI + layout) | `EvidenceAssertion` with `artifact_hash` |
| Layout spans are page/bbox, not char-only | ODL spaced keys handled in parser adapter |
| Import gate is fail-closed (D127) | `import_eligible = false` until human go |
| Statistical-first reduces LLM cost | GLiNER + header-priority before any LLM call |
| Promotion boundary needs explicit user go | Review gate FSM state |
| Char-only is justified fallback, not full evidence | `span_type` field distinguishes |
| GSD complete-milestone race conditions | Rust type system prevents partial states |
| Onion layering must be enforced | `da-domain` has zero external deps |
| Structure gate weak IR is not structure-ready | PPR + community detection verify structure |
| Prediction resolvability ≠ gold F1 | Agent search quality tracked separately |
