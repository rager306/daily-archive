# ADR-041: Samyama EmbeddedClient + Cypher Hybrid + AgentRuntime + SONA

**Status:** Accepted (binding)
**Date:** 2026-07-25
**Deciders:** collaborative
**Amends:** ADR-040 §3 (adapter strategy: HTTP → EmbeddedClient), ADR-037 §4.4 (agent: SymFSM from scratch → AgentRuntime + SONA)
**Binding Level:** binding

---

## 0. One-line Decision

> Ingest and batch operations use **direct GraphStore API** via `EmbeddedClient.store_write()` (100x faster than Cypher for bulk). Agent queries use **Cypher via EmbeddedClient** (cost-based planner, late materialization, plan cache). Simple user queries use **Samyama NLQ** (1 LLM call). The agent layer extends **Samyama AgentRuntime** with **RuVector SONA** learning loop — not built from scratch. HTTP adapter retained only for CLI and external tools.

---

## 1. Research Basis

Two studies conducted before this ADR:

### Study 1: RuVector vs Samyama Capability Matrix

Full inventory of both systems across 30+ categories. Key findings:

- **11 functions only in RuVector** (SONA, GNN-rerank, BM25+RRF, agent-memory, PPR, RVF, HNSW-repair, proof-gate, ruvllm, MaxSim, DiskANN)
- **11 functions only in Samyama** (AgentRuntime, NLQ, MCP auto-gen, GAK enrichment, late materialization, cost-based planner, multi-statement transactions, Raft+sharding, multi-tenancy, full ACID, GraphCatalog)
- **5 duplicates** (HNSW, graph CRUD, Cypher, PageRank, optimization solvers) — Samyama better in all 5

### Study 2: Cypher vs Direct SDK Performance

| Path | Latency | Components |
|------|---------|-----------|
| Cypher via HTTP | ~5-15ms | HTTP + parse (54%) + plan (44%) + execute (2%) |
| Cypher via EmbeddedClient | ~1-5ms | parse + plan + execute (no HTTP) |
| Direct GraphStore API | ~0.01-0.1ms | Rust method call (no parse/plan) |
| Direct VectorIndex API | ~0.5ms | HNSW search (no Cypher) |

Parse+plan = 98% of Cypher latency. Execution = 2%. Plan cache (v0.6.0) mitigates for repeated queries but first-call overhead remains.

---

## 2. Access Strategy: Three-Tier (binding)

```text
┌─────────────────────────────────────────────────────────────┐
│  da-application                                             │
│                                                            │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  HOT PATH: Direct GraphStore API                    │   │
│  │  EmbeddedClient.store_write() / store_read()       │   │
│  │  • Ingest (MERGE Paper + Author + CITES)            │   │
│  │  • Batch operations (60 papers)                     │   │
│  │  • Vector insert (VectorIndexManager.add_vector)    │   │
│  │  • Migration (GraphStore mutations)                 │   │
│  │  Latency: 0.01-0.1ms per operation                 │   │
│  │  No parse, no plan, no HTTP                         │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                            │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  WARM PATH: Cypher via EmbeddedClient               │   │
│  │  engine.execute(cypher, &store)                     │   │
│  │  • Agent MATCH queries (3-hop + WHERE + ORDER BY)   │   │
│  │  • Evidence verification (MATCH HAS_EVIDENCE)        │   │
│  │  • Graph health checks (orphan/dangling detection)  │   │
│  │  • Algorithm calls (CALL algo.pageRank)             │   │
│  │  Latency: 1-5ms (plan cache for repeats)            │   │
│  │  Cost-based planner + late materialization           │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                            │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  COLD PATH: Cypher via HTTP (RemoteClient)          │   │
│  │  • CLI tools (da health, da query)                  │   │
│  │  • External clients (debugging, ad-hoc)             │   │
│  │  • NLQ (simple user queries → LLM → Cypher)         │   │
│  │  Latency: 5-15ms (acceptable for interactive)       │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Binding rules

1. **Ingest pipeline** (`da-application::IngestUseCase`) → HOT path (direct API). Zero Cypher for batch writes.
2. **Agent queries** (`da-agent`, future) → WARM path (Cypher embedded). Declarative, planner-optimized.
3. **CLI/external** (`da-cli`) → COLD path (HTTP). Tolerable latency for interactive.
4. **Vector search** → HOT path for batch embed, WARM path for agent candidate retrieval.
5. **Algorithm calls** → WARM path (only Cypher CALL interface available).

---

## 3. da-adapters: HTTP → EmbeddedClient (binding)

### Current state

`SamyamaGraphStore` in `da-adapters/src/samyama_graph.rs` uses HTTP (`reqwest`) to `:8080`. This adds 5-15ms per query (HTTP serialize + deserialize).

### New state

`SamyamaGraphStore` wraps `samyama_sdk::EmbeddedClient` directly. No HTTP. No network. Same process.

```rust
// da-adapters/src/samyama_graph.rs (updated)

use samyama_sdk::{EmbeddedClient, SamyamaClient};
use samyama_sdk::embedded::AlgorithmClient;

pub struct SamyamaGraphStore {
    client: EmbeddedClient,
}

impl SamyamaGraphStore {
    pub fn new() -> Self {
        Self {
            client: EmbeddedClient::new(),
        }
    }

    /// Direct GraphStore access for hot-path operations.
    pub fn store(&self) -> &Arc<RwLock<GraphStore>> {
        self.client.store()
    }

    /// Direct vector index manager.
    pub fn vector_manager(&self) -> &VectorIndexManager {
        // Accessed via store_read() → graph_store.vector_index
    }
}
```

### Migration path

1. Add `samyama-sdk` as path dependency in `da-adapters/Cargo.toml`
2. Replace `reqwest::Client` with `EmbeddedClient`
3. `GraphStore::query()` → `client.query(graph, cypher).await` (embedded, no HTTP)
4. Add direct API methods: `create_paper_direct()`, `vector_search_direct()`
5. Keep HTTP adapter as `SamyamaHttpStore` (deprecated, for external tools only)

### Performance impact

| Operation | HTTP (current) | EmbeddedClient (new) | Speedup |
|-----------|:---:|:---:|:---:|
| Ingest 1 paper | ~15ms | ~0.1ms | **150x** |
| Ingest 60 papers | ~900ms | ~6ms | **150x** |
| Agent 3-hop query | ~10ms | ~2ms | **5x** |
| Vector search k=10 | ~8ms | ~0.5ms | **16x** |
| Health check | ~5ms | ~0.01ms | **500x** |

---

## 4. Agent Architecture: AgentRuntime + SONA (binding)

### Previous design (ADR-037 §4.4)

SymFSM built from scratch: Planning→Searching→Reading→Synthesis→Review→Repair→Output→Learning. All tools self-written.

### New design

Extend Samyama's built-in `AgentRuntime` with RuVector SONA learning loop.

```rust
// da-agent/src/lib.rs (future crate)

use samyama::agent::{AgentRuntime, AgentConfig};
use samyama::agent::tools::CypherTool;
use ruvector_sona::SonaEngine;
use ruvector_gnn_rerank::reranker::GnnDiffusionReranker;
use ruvector_hybrid::Bm25Index;

pub struct DailyArchiveAgent {
    // Samyama built-in: tool execution + telemetry + NLQ
    runtime: AgentRuntime,
    
    // RuVector: learning + reranking + keyword search
    sona: SonaEngine,
    gnn_rerank: GnnDiffusionReranker,
    bm25: Bm25Index,
    
    // Agent experience (RVF container)
    experience: RvfExperienceStore,
}
```

### Query flow (three tiers)

```text
User query
  │
  ├── is simple? ("papers citing 1206.6423")
  │   → Samyama NLQ (1 LLM call → Cypher → result)
  │   → ~70% of queries, cheapest path
  │
  ├── is complex? ("compare methods for link prediction on KGs")
  │   → AgentRuntime.plan_and_execute(question, tools)
  │   │   Tools: CypherTool (graph), VectorSearchTool (semantic),
  │   │          WebSearchTool (external), NLQClient (fallback)
  │   │   Telemetry: (:Question)-[:USED_TOOL]->(:Tool) edges auto-written
  │   → RuVector GNN-rerank on candidates
  │   → RuVector BM25 keyword boost
  │   → ~25% of queries, medium cost
  │
  └── needs evidence? ("prove this claim with page/bbox")
      → Full SymFSM: Planning→Search→Reading→Synthesis→Review
      → EvidenceStore.verify_chain()
      → ~5% of queries, highest cost, highest accuracy
```

### Learning loop (SONA integration)

```text
After every query (any tier):
  1. Extract trajectory from AgentRuntime telemetry
     (Question node + USED_TOOL edges + timing/cost)
  2. Compute reward:
     - explicit: user rating (👍/👎)
     - implicit: did user click through to cited paper?
     - metric: evidence resolvability score
  3. SONA: sona.end_trajectory(trajectory_builder, reward)
     → MicroLoRA adapter updated (<100μs)
     → EWC++ Fisher matrix updated (background)
  4. Next query: SONA adjusts candidate scores
     → Better retrieval without retraining model
  5. Periodically: flush to RVF experience container
     → Portable, signed, transferable
```

### What Samyama AgentRuntime provides (we don't build)

| Feature | Samyama built-in | Our addition |
|---------|:---:|:---:|
| CypherTool (graph query) | ✅ | — |
| WebSearchTool (external) | ✅ | — |
| NLQClient (LLM→Cypher) | ✅ | — |
| PlanExecutor (parallel/sequential) | ✅ | — |
| Telemetry edges | ✅ `(:Question)-[:USED_TOOL]->(:Tool)` | — |
| Safety (destructive rejection) | ✅ | — |
| Rate limiting | ✅ | — |
| **Learning loop** | ❌ | **RuVector SONA** |
| **GNN reranking** | ❌ | **RuVector GNN-rerank** |
| **BM25 keyword** | ❌ | **RuVector BM25+RRF** |
| **Memory compaction** | ❌ | **RuVector agent-memory** |
| **Experience persistence** | ❌ | **RVF container** |
| **Evidence verification** | ❌ | **EvidenceStore port** |

---

## 5. RuVector Narrowed Role (confirmed, ~8 crates)

After the capability matrix analysis, RuVector's irreducible role:

| Crate | Function | Why Samyama can't replace |
|-------|----------|--------------------------|
| `sona` | Self-learning (REINFORCE + MicroLoRA + EWC++) | Samyama has no learning loop |
| `ruvector-gnn-rerank` | Graph-aware candidate reranking (4 variants) | Samyama has global PageRank only |
| `ruvector-hybrid` | BM25 + RRF keyword+dense fusion | Samyama has no BM25 |
| `ruvector-agent-memory` | Memory compaction (LRU/LFU/coherence) | Samyama has telemetry, not compaction |
| `ruvector-solver` | PPR ForwardPush (personalized influence) | Samyama has global PageRank only |
| `rvf-runtime` | Agent experience container (COW + Ed25519) | Samyama .sgsnap = full-graph, not experience |
| `rvf-types` | Experience schema | — |
| `rvf-crypto` | Ed25519 signing | — |

**Eliminated from RuVector** (Samyama covers): HNSW (duplicate), GraphDB/redb (Samyama RocksDB), Cypher/rvlite (Samyama Cypher), optimization solvers (Samyama in-database), DiskANN (Phase 5+, optional).

**Healing crates** (optional, Phase 3+): `ruvector-hnsw-repair`, `ruvector-proof-gate`.

Total: **8 core + 2 optional = ~10 RuVector crates** (confirmed, unchanged from ADR-040).

---

## 6. Synergy Patterns (top 3 from ADHD analysis)

### Pattern A: AgentRuntime + SONA (★ recommended)

Samyama provides tool execution + telemetry. RuVector provides learning. Together: self-improving agent.

```rust
// After query completes
let telemetry = runtime.get_telemetry(question_id);
let trajectory = extract_trajectory(&telemetry);
let reward = compute_reward(&result);  // user feedback or metric
sona.end_trajectory(trajectory, reward);
// Next query: sona adjusts scores → better candidates
```

### Pattern B: NLQ candidate generation + RuVector precision reranking

Samyama NLQ → fast candidate set (recall). RuVector BM25 + GNN-rerank → precision top-K.

### Pattern C: .sgsnap (knowledge) + RVF (experience) dual persistence

Graph rollback without losing agent experience. Agent transfer between graph instances.

---

## 7. Binding Rules

1. **Ingest uses direct GraphStore API** — no Cypher for batch writes
2. **Agent queries use Cypher via EmbeddedClient** — cost-based planner optimizes
3. **Simple queries use NLQ** — 1 LLM call instead of full FSM
4. **HTTP adapter deprecated** — retained only for CLI/external tools
5. **AgentRuntime extended, not replaced** — RuVector SONA wraps Samyama's runtime
6. **RuVector never stores the full graph** — operates on subgraphs + working set
7. **RVF stores agent experience only** — not knowledge graph data
8. **import_eligible = false** (D127) — unchanged

---

## 8. Updated Crate Dependency Graph

```text
da-domain          (std + serde)
  ↑
da-ports           (da-domain)
  ↑
da-application     (da-domain + da-ports)
  ↑
da-adapters
  ├── samyama-sdk::EmbeddedClient  (direct GraphStore + Cypher embedded)
  ├── GrobidParser                 (HTTP to :8070)
  └── FdApiEmbedder                (HTTP to :8000)
  ↑
da-agent (future)
  ├── samyama::agent::AgentRuntime (built-in)
  ├── ruvector-sona                (learning loop)
  ├── ruvector-gnn-rerank          (candidate reranking)
  ├── ruvector-hybrid              (BM25 + RRF)
  ├── ruvector-agent-memory        (compaction)
  └── rvf-runtime                  (experience container)
  ↑
da-cli             (da-application + da-adapters + HTTP fallback)
```

---

## 9. LLM Reading Notes

- **Three-tier access**: direct API (hot, 0.01ms) → Cypher embedded (warm, 1-5ms) → Cypher HTTP (cold, 5-15ms).
- **Ingest migrates from HTTP to EmbeddedClient** — 150x speedup for batch.
- **Agent layer builds on Samyama AgentRuntime** — not from scratch. RuVector SONA wraps it.
- **NLQ for simple queries** — 70% of queries need only 1 LLM call.
- **RuVector irreducible: 8 crates** — SONA, GNN-rerank, BM25, agent-memory, PPR, RVF×3. Everything else is Samyama.
- **Cypher is NOT eliminated** — it's the WARM path for agent queries. Direct API is the HOT path for batch.
- **Plan cache mitigates Cypher overhead** for repeated agent queries.
- **Cost-based planner + late materialization** are free optimizations — we don't write manual traversal code.
