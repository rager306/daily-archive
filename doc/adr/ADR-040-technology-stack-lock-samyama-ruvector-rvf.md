# ADR-040: Technology Stack Lock — Samyama Graph + RuVector Agent Brain + RVF Experience

**Status:** Accepted (binding)
**Date:** 2026-07-25
**Deciders:** collaborative
**Supersedes:** ADR-037 §3 graph choice (NebulaGraph → Samyama Graph), ADR-037 §5 schema nGQL → Cypher
**Amends:** ADR-038 (NebulaGraph nGQL → Samyama Cypher), ADR-039 (graph store lifecycle updated)
**Binding Level:** binding
**Revisable:** no — technology selection is locked unless a binding ADR with milestone evidence supersedes

---

## 0. One-line Decision

> **Samyama Graph** (Rust-native, Apache 2.0) is the sole knowledge graph + vector + persistence engine. **RuVector** narrows to agent brain only (SONA, GNN-rerank, BM25, agent memory). **RVF** is the agent experience container (signed, portable, COW-branched). No NebulaGraph, no redb, no JVM, no second graph store.

---

## 1. Three-Tier Separation (binding)

```text
┌─────────────────────────────────────────────────────┐
│  TIER 1: KNOWLEDGE (Samyama Graph, embedded)         │
│  What we know about the world                        │
│  • Graph: Papers, Authors, Citations, Entities       │
│  • Cypher: ~90% OpenCypher (PEG + Volcano-Vectorized)│
│  • HNSW vectors: per-node, Cosine/L2/DotProduct      │
│  • Algorithms: PageRank, BFS, Dijkstra, WCC, SCC,    │
│    CDLP, LCC, TriangleCount, Edmonds-Karp, MST, PCA  │
│  • Persistence: RocksDB + WAL + CRC32                │
│  • MVCC: versioned-arena (get_node_at_version)       │
│  • Multi-tenancy: resource quotas per tenant         │
│  • Agent runtime: CypherTool, WebSearchTool, NLQ     │
│  • MCP: auto-generated from schema                   │
│  • GPU: PageRank/CDLP/LCC/Triangle/PCA via wgpu      │
│  • SDK: Rust embedded (zero overhead) + HTTP remote  │
│  • Proven: 74M nodes / 1B edges (PubMed scientific)  │
├─────────────────────────────────────────────────────┤
│  TIER 2: INTELLIGENCE (RuVector, agent brain crates)      │
│  How we search and reason per-query                  │
│  • SONA: self-learning retrieval (MicroLoRA + EWC++)  │
│  • GNN-rerank: GnnDiffusion, GnnMincut, ExactL2      │
│  • BM25 + RRF: hybrid keyword+dense fusion           │
│  • Agent memory: LRU/LFU/coherence compaction        │
│  • PPR: ForwardPushSolver on CSR from Samyama subgraph│
│  Operates on subgraphs pulled from Samyama — never   │
│  stores the full graph itself.                       │
├─────────────────────────────────────────────────────┤
│  TIER 3: EXPERIENCE (RVF runtime store)               │
│  What we learned from past interactions               │
│  • Trajectories: SymFSM paths (query→answer+reward)  │
│  • SONA weights: MicroLoRA adapters + Fisher matrix  │
│  • Agent memory: compacted entries (post-compaction)  │
│  • Reward history: per-query-type reward tracking     │
│  • Ed25519 signed: tamper-proof provenance            │
│  • COW branchable: fork per-domain without mutation   │
│  • Single-file portable: transfer between machines    │
│  • Boot as service: load experienced agent in 125ms   │
│  Persisted via `rvf-runtime` RvfStore (.rvf file)     │
│  This is case-based process memory, NOT model weights │
│  (DSPy/GRPO guard from ADR-039 rule #8 preserved).    │
└─────────────────────────────────────────────────────┘
```

---

## 2. Why Samyama Graph (not NebulaGraph / ArcadeDB / FalkorDB)

| Criterion | Samyama | NebulaGraph | ArcadeDB | FalkorDB |
|-----------|:-------:|:-----------:|:--------:|:--------:|
| **Language** | ✅ Rust | ❌ C++ | ❌ Java/JVM | ❌ C |
| **Last active** | 2026-07-07 | stale 2yr | 2026-04 | 2026-04 |
| **Proven scale** | **74M/1B** | distributed | ~3.2M | Redis limit |
| **Scientific KG** | ✅ PubMed 66M | — | research case | — |
| **Vector search** | ✅ HNSW built-in | external ES | ✅ JVector | ❌ |
| **Cypher** | ~90% OpenCypher | nGQL | OpenCypher 25 | ✅ |
| **Graph algorithms** | ✅ 12+ (rayon+GPU) | ✅ | ✅ LDBC leader | limited |
| **Persistence** | RocksDB+WAL | custom | custom | Redis |
| **Rust SDK** | ✅ embedded+HTTP | nebula-rs (3rd) | ❌ JVM | ✅ Rust client |
| **MCP** | ✅ auto-gen | ❌ | via LangChain | ❌ |
| **NLQ** | ✅ LLM→Cypher | ❌ | ❌ | ❌ |
| **graphrag-rs** | ✅ companion | ❌ | ❌ | ❌ |
| **Docker** | 1 container | segfault (139) | JVM | Redis module |
| **License** | Apache 2.0 | Apache 2.0 | Apache 2.0 | SSPL |

**Samyama wins on:** Rust-native, proven at scale on scientific papers, built-in vectors + algorithms + Cypher, graphrag-rs companion, single binary deployment.

---

## 3. What Samyama Replaces (binding inventory)

### From RuVector — eliminated for KNOWLEDGE GRAPH storage (Samyama covers)

These RuVector components were used for runtime graph/vector **knowledge** storage. Samyama replaces them entirely for this purpose. They are NOT used for agent experience storage (see "kept" below).

| RuVector component | Samyama replacement | Status |
|-------------------|--------------------|----|
| `ruvector-graph` (GraphDB, redb) | `GraphStore` (RocksDB) | ✅ full |
| `ruvector-core` HNSW VectorDB | `VectorIndex` + `VectorIndexManager` | ✅ full |
| `ruvector-graph` Cypher (stub) | Samyama ~90% working Cypher | ✅ upgrade |
| `ruvector-solver` PPR (for influence) | `page_rank()` (global) + build CSR + ForwardPush (personalized) | ✅ via adapter |
| `ruvector-graph-algorithms` | `samyama-graph-algorithms` crate | ✅ full |
| `rvlite` Cypher executor | Samyama Cypher engine (PEG + Volcano) | ✅ upgrade |

> **NOT eliminated:** `rvf-runtime` RvfStore. While RVF is no longer used for *knowledge graph* runtime storage (Samyama RocksDB covers that), **RVF runtime stays as the persistence layer for agent experience** (Tier 3). See "kept" below.

### From RuVector — kept (agent brain + experience + healing, ~10 crates)

| Crate | Role | Why kept |
|-------|------|----------|
| `sona` | Self-learning (MicroLoRA, EWC++, trajectory buffer) | Unique; Samyama has no equivalent |
| `ruvector-gnn-rerank` | Graph-aware candidate reranking | Unique; Samyama has no GNN rerank |
| `ruvector-hybrid` (BM25+RRF) | Keyword+dense fusion on working set | Unique; Samyama has no BM25 |
| `ruvector-agent-memory` | Memory compaction (LRU/LFU/coherence) | Unique; Samyama has tenancy, not memory compaction |
| `ruvector-solver` (ForwardPush) | PPR on CSR subgraphs | Used for personalized influence; Samyama has global PageRank only |
| **`rvf-runtime`** (CowEngine, RvfStore) | **Agent experience persistence** — stores trajectories, SONA weights, compacted memory in a portable, signed, COW-branched `.rvf` file | Unique: COW branching + Ed25519 signing + single-file portability for agent experience. Samyama has no equivalent for experience containerization. |
| **`rvf-types`** | Experience container schema/types | Defines the agent experience data model |
| **`rvf/rvf-crypto`** | Ed25519 signing for experience provenance | Tamper-proof agent experience audit |
| **`ruvector-hnsw-repair`** | **HNSW vector index healing** after deletions (TombstoneOnly / BatchRepair / EagerRepair) | Preserves recall when paper embeddings deleted/updated; applicable to Samyama VectorIndex |
| **`ruvector-proof-gate`** | **Write provenance** — cryptographic receipts on every write (HashChainGate ~200ns, MerkleGate ~300ns) | Tamper-evident audit trail for agent experience writes; offline-verifiable |

---

## 4. Component → ADR Cross-Reference

| Component | Defined in | Status | Lifecycle |
|-----------|-----------|--------|-----------|
| Hexagonal 6-layer | ADR-037 §2 | binding | `[validated]` pattern |
| Samyama Graph (graph+vector+persist) | **ADR-040 §1 Tier 1** | binding | `[bounded]` (smoke proven, pipeline pending) |
| RuVector agent brain (SONA+rerank+BM25) | **ADR-040 §1 Tier 2** | binding | `[proposed]` (integration unproven) |
| RVF experience container | **ADR-040 §1 Tier 3** | binding | `[proposed]` (Phase 4+) |
| GROBID + ODL parsers | ADR-037 §4.1 | binding | `[validated]` (60 canary) |
| GLiNER 2 offline NER | ADR-038 §3 | binding | `[bounded]` (1 paper smoke) |
| Core-then-Modes extraction | ADR-038 §3 | binding | `[proposed]` |
| Tri-source retrieval | ADR-038 §4 | binding | `[proposed]` |
| 6 graph operators O1-O6 | ADR-038 §5 | binding | `[proposed]` |
| Agents-K1 5-module schema | ADR-038 §2 | binding | `[proposed]` |
| 18 relation types | ADR-038 §2 | binding | `[proposed]` |
| SHA256 stable VIDs | ADR-038 §6 | binding | `[bounded]` |
| Hyperedge ExperimentSetup | ADR-038 §7 | binding | `[proposed]` |
| SymFSM agent (7 states + REVIEW) | ADR-037 §4.4, ADR-039 §6 | binding | `[proposed]` |
| Versioned\<T\> temporality | ADR-037 §6 | binding | `[proposed]` |
| Lifecycle tags | ADR-039 §1 | binding | `[validated]` (process) |
| Staged validation gates | ADR-039 §2 | binding | `[validated]` (process) |
| Import gate fail-closed (D127) | ADR-039 §6 | binding | `[validated]` |
| Statistical-first | ADR-037 §4.2, ADR-036 | binding | `[validated]` |
| Evidence chain immutable | ADR-037 §9 | binding | `[validated]` |
| LLM rate-limit-aware | ADR-037 §4.3 | binding | `[validated]` |

---

## 5. Environment Configuration (locked)

### Infrastructure services

| Service | Port | Status | Config |
|---------|------|--------|--------|
| **Samyama Graph** | RESP 6380, HTTP 8080 | ✅ running (smoke) | `vendor-source/samyama-graph` |
| **GROBID** | 8070 | ✅ running | `.docker/docker-compose.yml` |
| **ODL (OpenDataLoader)** | subprocess | ✅ installed | `opendataloader-pdf` |
| **GLiNER 2** | subprocess | ✅ installed | `gliner2[local]` |
| **9router (LLM proxy)** | 20128 | ✅ running | 347 models available |
| **TEI embedder (fd_api)** | 8000 | ✅ running | bge-m3, 1024d |
| **RuVector** | path dep | ✅ vendored | `vendor-source/ruvector` |

### LLM model roles (locked, from GSD memory + preferences)

| Role | Model | Endpoint | Notes |
|------|-------|----------|-------|
| Fast default | `agnes-ai/agnes-2.0-flash` | 9router `/v1/chat/completions` | OpenAI-compatible |
| Quality | `MiniMax-M2.7-highspeed` | `api.minimax.io/anthropic/v1/messages` | **X-Api-Key** header (not Bearer); GOTCHA: MINIMAX_API_KEY must match ANTHROPIC_API_KEY value |
| Fallback | `grok-4.5` | 9router | when primary rate-limited |
| Judge | `glm-5.2` / `gpt-5.2` | 9router | verification, checker model |

### .env changes (NebulaGraph → Samyama)

Replaced `NEBULA_GRAPH_*` vars with:
```env
SAMYAMA_RESP_URL=127.0.0.1:6380
SAMYAMA_HTTP_URL=http://127.0.0.1:8080
SAMYAMA_DATA_DIR=data/samyama
SAMYAMA_DEFAULT_TENANT=daily_archive
```

---

## 6. Python Lessons Preserved (from GSD memory store)

These are `[validated]` process rules from M001–M284 and MUST be reflected in the Rust implementation:

| Lesson | Source | ADR-040 binding rule |
|--------|--------|---------------------|
| MiniMax Anthropic endpoint uses `X-Api-Key`, not Bearer; stale key causes 401 | GSD gotcha | §5 LLM model roles — documented with gotcha |
| Graph-readiness review must run before promotion | GSD convention | ADR-039 §6 REVIEW state — post-check before manifest |
| Fixture-level hybrid retrieval ≠ production corpus retrieval | M003 S06 | ADR-039 rule #3: fixture ≠ production |
| ODL → typed adapter success ≠ semantic quality or graph readiness | M033 S07 | ADR-039 rule #4: adapter success ≠ readiness |
| DSPy/GRPO guarded until S07 metrics verified | M003 S07 | ADR-039 rule #8: optimizer guard |
| Pause feature expansion, validate on real batches | M003 | ADR-039 §2: staged validation gates |
| Hybrid body route: decide_body_route + resolve_article_body (ADR-008/009) | M211 | Samyama GraphStore stores the result; routing logic in da-application |
| Loader batch selection optional, separate from core contract | GSD architecture | da-application/ingest — optional capability, not core port |
| Closeout verification uses implemented flags while preserving contracts | M022 S03 | ADR-039 lifecycle: bounded→validated requires real-batch evidence |
| ADR template: prose authoritative, Mermaid optional, LLM Reading Notes mandatory | M034 convention | This ADR includes LLM Reading Notes (§9) |

---

## 7. Schema Migration (NebulaGraph nGQL → Samyama Cypher)

ADR-038 schema was written for NebulaGraph nGQL. Samyama uses OpenCypher. The schema definitions translate:

| NebulaGraph nGQL | Samyama Cypher |
|------------------|---------------|
| `CREATE TAG Paper(...)` | `CREATE (n:Paper {prop: value})` |
| `CREATE EDGE CITES(...)` | `CREATE (a)-[:CITES {prop: value}]->(b)` |
| `MATCH (n:Paper) WHERE ...` | same (OpenCypher compatible) |
| `GET SUBGRAPH 3 STEPS FROM ...` | SDK `build_view()` + `bfs()` |
| `CREATE SPACE daily_archive(...)` | tenant: `daily_archive` (built-in multi-tenancy) |

**ADR-038 schema is still binding** — the 5-module (A-E) structure, 18 relation types, CitationContext-as-node, multimodal nodes all carry over. Only the DDL syntax changes from nGQL to Cypher. Implementation will use Samyama Cypher CREATE statements.

---

## 8. Crate Dependency Graph (locked)

```text
da-domain          (std + serde only)
  ↑
da-ports           (da-domain)
  ↑
da-application     (da-domain + da-ports)
  ↑                ↑
da-adapters        da-graph (samyama-sdk + algorithms)
  ↑                ↑
da-cli             (da-application + da-adapters + da-graph)

External path deps:
  samyama-sdk      → vendor-source/samyama-graph/crates/samyama-sdk
  samyama          → vendor-source/samyama-graph (lib)
  ruvector-sona    → vendor-source/ruvector/crates/sona
  ruvector-gnn-rerank → vendor-source/ruvector/crates/ruvector-gnn-rerank
  ruvector-hybrid  → vendor-source/ruvector/crates/ruvector-hybrid
  ruvector-agent-memory → vendor-source/ruvector/crates/ruvector-agent-memory
  ruvector-solver  → vendor-source/ruvector/crates/ruvector-solver
  rvf-runtime      → vendor-source/ruvector/crates/rvf/rvf-runtime (agent experience persistence)
  rvf-types        → vendor-source/ruvector/crates/rvf/rvf-types
  rvf-crypto       → vendor-source/ruvector/crates/rvf/rvf-crypto
  ruvector-hnsw-repair → vendor-source/ruvector/crates/ruvector-hnsw-repair (HNSW index healing)
  ruvector-proof-gate  → vendor-source/ruvector/crates/ruvector-proof-gate (write provenance crypto)
```

---

## 9. LLM Reading Notes

- **Binding:** Samyama Graph is the sole graph+vector+persist engine. RuVector is agent brain only. RVF is experience container.
- **Superseded by this ADR:** ADR-037 NebulaGraph choice, ADR-037 redb/RVF runtime storage role, ADR-038 NebulaGraph nGQL syntax (schema structure preserved).
- **Still binding:** ADR-037 hexagonal structure + SymFSM + versioning; ADR-038 schema/modules/operators/extraction; ADR-039 lifecycle discipline.
- **Samyama proven at:** 74M nodes / 1B edges (PubMed), 255K nodes/s ingestion, 115K QPS at 1M, 100% LDBC Graphalytics (28/28).
- **RuVector narrowed to:** SONA, GNN-rerank, BM25+RRF, agent-memory, PPR ForwardPush + RVF + healing (~10 crates total, not 160).
- **RVF role:** agent experience container (trajectories + SONA weights + compacted memory + signed). Case-based process memory, not model fine-tuning. DSPy/GRPO guard preserved.
- **Phase ordering:** Phase 1-3 = Samyama + domain + ingest + extraction. Phase 4 = RuVector agent brain + RVF experience. RVF is not blocking.
- **Import gate:** `import_eligible = false` until explicit human yes (D127). No exceptions.
- **Optimization stack (§10):** Python DSPy eliminated. Rust-native: `gepa` (prompt evolution, ICLR 2026), RuVector SONA (REINFORCE + MicroLoRA retrieval learning), `dsrust` (full DSPy parity), `productioneer/dspy-rs` (GRPO included), samyama-optimization (NSGA-II). Guard ADR-039 rule #8 preserved: no optimizer before Stage 2+ metrics.

---

## 11. Schema, Versioning, Migration, and Graph Healing

### 11.1 Schema enforcement (application-layer, not DDL)

Samyama is schemaless: `CREATE (n:Paper {anything})` accepts any properties. No DDL constraints (NOT NULL, UNIQUE, type enforcement). This is a feature, not a limitation — schema lives in the **domain layer** (`da-domain`), enforced by Rust types.

**Pattern:** Schema-as-code with compile-time type checking.

```rust
// da-domain/src/schema.rs — single source of truth

pub struct PaperSchema;

impl NodeSchemaDef for PaperSchema {
    fn label() -> &'static str { "Paper" }
    fn required() -> &'static [Field] {
        &[Field::string("vid"), Field::string("arxiv_id"),
          Field::string("title"), Field::datetime("valid_from")]
    }
    fn optional() -> &'static [Field] {
        &[Field::datetime("valid_to"), Field::integer("version").default(1),
          Field::boolean("import_eligible").default(false)]
    }
    fn validate(props: &PropertyMap) -> Result<(), SchemaError> { /* type-check each field */ }
}
```

This generates: `validate()` (pre-write check), `to_properties()` (Rust→PropertyValue), `from_properties()` (PropertyValue→Rust, with type mismatch detection), `to_cypher_create()` (Cypher fragment).

**Why better than DDL:** type errors caught at **compile-time** (Rust compiler), not runtime (INSERT fails). Schema evolution = change struct + recompile, not `ALTER TAG` migration.

### 11.2 Schema versioning

Every node carries a `schema_version` property:

```cypher
CREATE (n:Paper {
    vid: "sha256...",
    arxiv_id: "1206.6423",
    schema_version: 1,    -- incremented when schema changes
    ...
})
```

The application tracks the current schema version (`da-domain::CURRENT_SCHEMA_VERSION`). On startup, it checks `MAX(schema_version)` across nodes. If mismatch → migration needed.

### 11.3 Migration framework (da-application)

No built-in migration in Samyama or RuVector. We build a lightweight migration system:

```rust
// da-application/src/migration.rs

pub trait Migration {
    fn version(&self) -> u32;               // from_version
    fn description(&self) -> &str;
    fn up(&self, store: &EmbeddedClient) -> Result<MigrationStats>;
    fn down(&self, store: &EmbeddedClient) -> Result<()>;  // rollback
    fn validate(&self, store: &EmbeddedClient) -> Result<bool>;  // post-check
}

pub struct MigrationRunner {
    migrations: Vec<Box<dyn Migration>>,
    snapshot_before: bool,   // Samyama .sgsnap before each migration
}

impl MigrationRunner {
    pub fn run(&self, store: &EmbeddedClient) -> Result<MigrationReport> {
        // 1. Samyama snapshot export (rollback safety net)
        let snapshot = store.export_snapshot()?;
        
        // 2. Detect current schema version
        let current = store.query_one("MATCH (n) RETURN max(n.schema_version)")?;
        
        // 3. Apply migrations sequentially
        for m in self.migrations.iter().filter(|m| m.version() >= current) {
            m.up(store)?;           // apply
            if !m.validate(store)? { // post-check
                store.import_snapshot(&snapshot)?;  // rollback
                return Err(MigrationError::ValidationFailed);
            }
        }
        Ok(report)
    }
}
```

**Migration examples:**

| From | To | What changes | Cypher |
------|----|-------------|--------|
| v1 | v2 | Add `doi` field to Paper | `MATCH (n:Paper) SET n.doi = null, n.schema_version = 2` |
| v2 | v3 | Rename `abstract` → `abstract_text` | `MATCH (n:Paper {abstract: $old}) SET n.abstract_text = n.abstract, n.schema_version = 3` |
| v3 | v4 | Add `embedding` vector to all Paper nodes | Batch: embed abstract → `SET n.embedding = $vec` |
| v4 | v5 | Split `Entity` label into `Method`/`Dataset`/`Model` | `MATCH (n:Entity {type: "method"}) SET n:Method REMOVE n:Entity` |

### 11.4 Graph healing (integrity checks)

Samyama has no built-in integrity checker. We implement graph health checks at three layers:

#### Layer 1: RuVector healing crates (available, reusable)

| Crate | What it heals | How | Use in daily-archive |
|-------|-------------|-----|---------------------|
| `ruvector-hnsw-repair` | **HNSW vector proximity graph** after vector deletion | 3 strategies: TombstoneOnly (mark, recall degrades), BatchRepair (periodic sweep), EagerRepair (reconnect immediately) | Applicable to Samyama VectorIndex — when paper embeddings are deleted/updated, HNSW graph degrades; this crate reconnects neighbors to preserve recall |
| `ruvector-proof-gate` | **Write provenance** — every write gets a cryptographic receipt | NullGate (dev), HashChainGate (~200ns, sequential tamper-evidence), MerkleGate (~300ns, MMR membership proofs) | For RVF agent experience (Tier 3): every trajectory/weight write signed, verifiable offline |
| `ruvector-verified` | **Formal verification** via dependent types | Proof-carrying code: prove_dim_eq, VerifiedStage, ProofAttestation | Research-level, optional: formally prove pipeline stages preserve vector dimensions |

> **Key insight:** RuVector's healing operates on the **vector index layer** (HNSW proximity graph) and **write provenance layer** (crypto receipts), NOT on the knowledge graph layer (orphan Paper nodes, dangling CITES edges). Knowledge graph healing is our own Cypher-based system (Layer 2 below).

#### Layer 2: Knowledge graph healing (we build, Cypher-based)

```rust
// da-application/src/graph_health.rs

pub struct GraphHealthReport {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub orphan_nodes: Vec<NodeId>,        // no edges at all
    pub dangling_edges: Vec<EdgeId>,      // source or target deleted
    pub missing_required: Vec<(NodeId, String)>,  // missing required property
    pub type_mismatches: Vec<(NodeId, String, String)>,  // wrong PropertyValue type
    pub schema_version_stale: Vec<NodeId>, // old schema_version
    pub vector_missing: Vec<NodeId>,      // Paper without embedding
    pub duplicate_vids: Vec<(String, Vec<NodeId>)>, // same SHA256 vid on multiple nodes
}

impl GraphHealthReport {
    pub fn check(store: &EmbeddedClient) -> Result<Self> {
        // Orphan nodes: no incoming or outgoing edges
        let orphans = store.query(
            "MATCH (n) WHERE NOT (n)--() RETURN n.vid"
        )?;
        
        // Missing required: Paper without arxiv_id
        let missing = store.query(
            "MATCH (n:Paper) WHERE n.arxiv_id IS NULL RETURN n.vid"
        )?;
        
        // Duplicate VIDs: same canonical identity on multiple nodes
        let dups = store.query(
            "MATCH (n) WITH n.vid AS vid, count(*) AS cnt WHERE cnt > 1 RETURN vid, cnt"
        )?;
        
        // Type mismatch: version stored as String instead of Integer
        // (checked via da-domain from_properties() validation on read-back)
        
        // Stale schema: nodes with schema_version < CURRENT
        let stale = store.query(
            "MATCH (n) WHERE n.schema_version < $current RETURN n.vid, n.schema_version"
        )?;
        
        // Vector missing: Paper without embedding
        let no_vec = store.query(
            "MATCH (n:Paper) WHERE n.embedding IS NULL RETURN n.vid"
        )?;
        Ok(report)
    }
    
    pub fn heal(&self, store: &EmbeddedClient) -> Result<HealReport> {
        // 1. Dedup by SHA256 vid: merge properties, redirect edges, delete duplicate
        // 2. Backfill missing vectors (re-embed via fd_api/OnnxEmbedder)
        // 3. Run migrations on stale schema_version nodes
        // 4. Delete dangling edges (source/target node missing)
        // 5. Report unhealable issues for manual review
        // 6. After HNSW vector changes: invoke ruvector-hnsw-repair for index healing
    }
}
```

#### Layer 3: Write-time prevention (schema validation + proof gate)

Before data enters the graph, three gates prevent corruption:

1. **Schema validation** (`da-domain`): `PaperSchema::validate(props)` checks required fields, types, ranges — rejects invalid data before Samyama CREATE
2. **Proof gate** (`ruvector-proof-gate`): every graph write gets a `WriteReceipt` — tamper-evident audit trail
3. **VID canonicalization** (`da-domain`): SHA256 identity ensures no duplicate entities — `paper_vid(arxiv_id)` is deterministic

#### Healing flow summary

```text
Write path (prevention):
  data → da-domain validate() → proof-gate admit() → Samyama CREATE
  (corrupt data never enters the graph)

Read path (detection):
  Samyama MATCH → da-domain from_properties() → SchemaError on type mismatch
  (corrupt data detected on read-back)

Periodic sweep (healing):
  GraphHealthReport::check() → Cypher queries → identify issues
  GraphHealthReport::heal() → dedup, backfill, migrate, prune
  ruvector-hnsw-repair → fix HNSW index after vector changes

Rollback (safety net):
  Samyama .sgsnap snapshot → import_snapshot() → full restore
```

### 11.5 Temporal versioning (data versioning, not schema)

Samyama MVCC is for **concurrency control** (snapshot isolation), not temporal queries ("what was the state at time X"). We implement data-level temporality:

```cypher
-- Paper v1 (preprint) → v2 (camera-ready)
CREATE (v2:Paper {arxiv_id: "1206.6423", version: 2, valid_from: timestamp()})
MATCH (v1:Paper {arxiv_id: "1206.6423", version: 1})
SET v1.valid_to = timestamp(), v1.superseded_by = $v2_vid
CREATE (v1)-[:SUPERSEDES {reason: "camera_ready"}]->(v2)

-- Query: what was the title on 2024-06-01?
MATCH (n:Paper {arxiv_id: "1206.6423"})
WHERE n.valid_from <= $as_of
  AND (n.valid_to IS NULL OR n.valid_to > $as_of)
RETURN n.title, n.version
```

Both versions coexist in the graph. Old versions are never deleted — `SUPERSEDES` edges link the chain.

### 11.6 Samyama snapshot for migration safety

Samyama `.sgsnap` format provides atomic backup/restore:

- **Before migration:** `store.export_snapshot()` → `.sgsnap` file
- **If migration fails:** `store.import_snapshot(file)` → full rollback
- **Atomic write:** tmp → fsync → rename → committed marker (crash-safe)

This replaces NebulaGraph's lack of snapshot capability.

### 11.7 RuVector GraphSchema (reference, optional opt-in)

RuVector has a `GraphSchema` module (`ruvector-graph/src/schema.rs`) that provides opt-in schema-first validation:

```rust
pub struct GraphSchema {
    nodes: HashMap<Label, NodeSchema>,    // required/optional props + types
    edges: HashMap<EdgeType, EdgeSchema>, // from/to label constraints
    vectors: HashMap<String, VectorSchema>, // label + property + dim + metric
}
```

`TypedGraph` wraps `GraphDB` and validates mutations before storage. This is useful for the RuVector agent brain working set (Tier 2), but for the main knowledge graph (Tier 1 Samyama), our `da-domain` schema-as-code is the authority.

### 11.8 Binding rules

1. **Schema lives in `da-domain`** — Rust structs, not DDL. Compile-time type safety.
2. **Every node has `schema_version`** — integer property tracking schema generation.
3. **Migrations are sequential + validated** — each migration has `up()`, `validate()`, `down()` (rollback via snapshot).
4. **Samyama snapshot before migration** — `.sgsnap` atomic backup as safety net.
5. **Graph health checks are Cypher queries** — orphan/dangling/missing/type-mismatch detection.
6. **Temporal versioning via SUPERSEDES edges** — old versions never deleted.
7. **No destructive migrations without snapshot** — `import_eligible = false` during healing.

---

## 12. Observability (binding)

### 12.1 Health endpoints

```rust
// da-cli/src/health.rs — unified health check

pub struct HealthReport {
    // Infrastructure services
    pub samyama: ServiceHealth,    // RESP 6380 + HTTP 8080
    pub grobid: ServiceHealth,     // HTTP 8070
    pub odl: ServiceHealth,        // subprocess availability
    pub gliner2: ServiceHealth,    // subprocess availability
    pub ninerouter: ServiceHealth, // HTTP 20128
    pub tei_embedder: ServiceHealth, // HTTP 8000 (fd_api)
    
    // Data health
    pub graph_stats: GraphStats,   // node_count, edge_count, vector_count
    pub schema_versions: SchemaVersionStats, // max schema_version, stale count
    pub health_issues: Vec<HealthIssue>, // from §11.4 GraphHealthReport
    
    // Agent health
    pub sona_stats: SonaStats,     // trajectory_count, avg_reward, lora_update_count
    pub agent_memory: MemoryStats, // entry_count, compaction_count
    
    // Lifecycle
    pub lifecycle_stage: LifecycleStage, // fixture | canary_10 | canary_60 | ...
    pub import_eligible: bool,     // always false until explicit human go
    pub evidence_ready_ok: bool,  // from evidence_dashboard
}

pub struct ServiceHealth {
    pub name: String,
    pub status: HealthStatus,      // healthy | degraded | down
    pub latency_ms: Option<u64>,
    pub last_check: i64,
    pub error: Option<String>,
}
```

**CLI:** `da health` → prints HealthReport
**HTTP (future):** `GET /health` → JSON

### 12.2 Metrics (Prometheus-ready)

| Metric | Type | Labels | What it tells you |
|--------|------|--------|------------------|
| `da_graph_nodes_total` | gauge | label | total nodes per label (Paper, Author, Entity, ...) |
| `da_graph_edges_total` | gauge | edge_type | total edges per type (CITES, AUTHORED, APPLIED_TO, ...) |
| `da_evidence_resolvability_rate` | gauge | metric_mode | resolvability rate (gold vs prediction) |
| `da_evidence_page_bbox_count` | gauge | — | nodes with page/bbox spans |
| `da_llm_calls_total` | counter | provider, model, purpose | LLM API calls (extraction, synthesis, upgrade) |
| `da_llm_tokens_total` | counter | provider, direction (in/out) | token usage |
| `da_gliner_entities_total` | counter | — | entities extracted by GLiNER 2 |
| `da_sona_trajectory_count` | gauge | — | trajectories stored |
| `da_sona_avg_reward` | gauge | — | rolling average reward |
| `da_etl_queue_depth` | gauge | task_type | pending ETL tasks per type |
| `da_etl_task_duration_seconds` | histogram | task_type | task latency distribution |
| `da_import_eligible` | gauge | — | 0 (locked) or 1 (human go — should never be 1 without audit) |
| `da_schema_version_max` | gauge | — | highest schema_version in graph |
| `da_schema_stale_nodes` | gauge | — | nodes below CURRENT_SCHEMA_VERSION |
| `da_graph_health_issues` | gauge | issue_type | orphan/dangling/missing/type-mismatch counts |

### 12.3 Alerting thresholds (binding)

| Alert | Condition | Severity | Action |
|-------|-----------|----------|--------|
| Samyama down | `samyama.status == down` | CRITICAL | Restart; if persistent, check RocksDB corruption |
| GROBID down | `grobid.status == down` | HIGH | ETL pipeline blocked; restart container |
| Resolvability drop | `evidence_resolvability_rate < 0.70` | HIGH | Check embedding quality; check layout JSON freshness |
| Schema stale | `schema_stale_nodes > 0` after migration | MED | Run migration runner |
| Import eligible | `import_eligible == 1` without audit | CRITICAL | **This should never happen** — investigate immediately |
| LLM rate limit | `llm_calls_failed > 0` | MED | Switch provider (9router → MiniMax → grok) |
| Graph health issues | `graph_health_issues > 10` | MED | Run `da health --heal` |
| Disk low | `disk_free_gb < 5` | HIGH | Pause ingest; cleanup artifacts |
| SONA reward declining | `sona_avg_reward trending < baseline - 0.1` | LOW | Check trajectory quality; possibly reset adapters |

### 12.4 Tracing (structured logs)

Every operation emits a structured log entry:

```rust
// tracing span pattern
#[tracing::instrument(skip(store), fields(paper_id, stage))]
async fn ingest_paper(store: &EmbeddedClient, paper_id: &str) -> Result<()> {
    tracing::info!(paper_id, stage = "fetch", "fetching PDF");
    // ...
    tracing::info!(paper_id, stage = "parse", "parsing via GROBID+ODL");
    // ...
    tracing::info!(paper_id, stage = "extract", "GLiNER entities", count = entities.len());
    // ...
    tracing::info!(paper_id, stage = "graph_write", "writing to Samyama", 
                   evidence_ready = true, import_eligible = false);
}
```

Log format: `RUST_LOG_FORMAT=full` (from `.env`).
Log levels per crate: `RUST_LOG_DA_DOMAIN=info`, `RUST_LOG_DA_AGENT=debug`.

---

## 13. EvidenceStore Port (binding)

### 13.1 Problem

ADR-037 defines EvidenceAssertion and SourceSpan as domain types. But there is no port that links a Samyama graph node (by VID) to its immutable evidence artifacts (PDF hash, TEI, ODL layout, page/bbox spans).

### 13.2 EvidenceStore trait

```rust
// da-ports/src/evidence_store.rs

/// Links graph nodes to immutable evidence artifacts.
/// Every Entity/Relation node MUST have at least one EvidenceAssertion
/// resolvable through this port before import_eligible can be considered.
#[async_trait]
pub trait EvidenceStore: Send + Sync {
    /// Store an evidence assertion for a graph node.
    async fn store_assertion(
        &self,
        node_vid: &str,
        assertion: &EvidenceAssertion,
    ) -> Result<EvidenceId>;

    /// Retrieve all evidence assertions for a node.
    async fn get_assertions(&self, node_vid: &str) -> Result<Vec<EvidenceAssertion>>;

    /// Check if a node has resolvable evidence (page/bbox or char span).
    async fn has_resolvable_evidence(&self, node_vid: &str) -> Result<bool>;

    /// Verify evidence chain integrity: artifact_hash exists on disk,
    /// span coordinates are within document bounds.
    async fn verify_chain(&self, node_vid: &str) -> Result<EvidenceVerification>;

    /// Batch check: which nodes lack evidence? (for graph health)
    async fn nodes_without_evidence(&self, label: &str) -> Result<Vec<String>>;
}

pub struct EvidenceAssertion {
    pub claim: String,
    pub span_type: SpanType,        // PageBbox | CharOnly | Tei
    pub page: Option<u32>,
    pub bbox: Option<[f64; 4]>,
    pub char_start: Option<usize>,
    pub char_end: Option<usize>,
    pub artifact_hash: String,     // SHA256 of PDF/TEI/ODL
    pub artifact_path: String,     // filesystem path to immutable artifact
    pub epistemic_status: EpistemicStatus, // Verified | Staged | Pending
    pub created_at: i64,
}

pub struct EvidenceVerification {
    pub node_vid: String,
    pub artifact_exists: bool,
    pub hash_matches: bool,
    pub span_in_bounds: bool,
    pub verdict: EvidenceVerdict,   // Valid | ArtifactMissing | HashMismatch | SpanOutOfBounds
}
```

### 13.3 Samyama adapter (implementation pattern)

Evidence is stored in two places:
1. **Samyama graph:** `(:Entity)-[:HAS_EVIDENCE]->(:EvidenceAssertion {artifact_hash, page, bbox, ...})`
2. **Filesystem:** immutable artifacts at `data/evidence/<artifact_hash_prefix>/<artifact_hash>.pdf|.tei|.layout.json`

```rust
// da-adapters/src/samyama_evidence_store.rs

pub struct SamyamaEvidenceStore {
    client: EmbeddedClient,
    evidence_root: PathBuf,  // data/evidence/
}

#[async_trait]
impl EvidenceStore for SamyamaEvidenceStore {
    async fn store_assertion(&self, vid: &str, assertion: &EvidenceAssertion) -> Result<EvidenceId> {
        // 1. Verify artifact exists on disk
        let artifact_path = self.evidence_root.join(&assertion.artifact_hash);
        if !artifact_path.exists() {
            return Err(EvidenceError::ArtifactMissing(assertion.artifact_hash.clone()));
        }

        // 2. Create EvidenceAssertion node in Samyama
        let cypher = format!(
            "MATCH (n {{vid: $vid}}) \
             CREATE (e:EvidenceAssertion {{
                 claim: $claim, span_type: $span_type,
                 page: $page, bbox: $bbox,
                 artifact_hash: $hash, artifact_path: $path,
                 epistemic_status: $status, created_at: $ts
             }}) \
             CREATE (n)-[:HAS_EVIDENCE]->(e) \
             RETURN e.vid"
        );
        // ...
    }

    async fn verify_chain(&self, vid: &str) -> Result<EvidenceVerification> {
        // 1. Get assertion from Samyama
        // 2. Check artifact file exists
        // 3. Recompute SHA256, compare to stored hash
        // 4. Check span bounds (page/bbox within document)
    }
}
```

### 13.4 Binding rules

1. **Every Entity/Relation node** must have at least one `HAS_EVIDENCE` edge to an `EvidenceAssertion` node before `import_eligible` can be considered.
2. **Artifact hashes are immutable** — PDF, TEI, ODL layout files are content-addressed by SHA256 and never modified.
3. **`verify_chain()` is called by the REVIEW FSM state** (ADR-039 §6) before any output.
4. **`nodes_without_evidence()`** feeds the GraphHealthReport (§11.4) for periodic integrity sweeps.
5. **Import gate (D127):** `import_eligible = false` even if all evidence is verified — human go is still required.
- **What is NOT adopted from RuVector:** GraphDB (redb for knowledge), VectorDB (HNSW for knowledge), Cypher stub, rvlite, ~150 other crates. Kept: 5 agent-brain crates (SONA, GNN-rerank, BM25, agent-memory, PPR) + 3 RVF crates (rvf-runtime, rvf-types, rvf-crypto) + 2 healing crates (ruvector-hnsw-repair, ruvector-proof-gate). Total: ~10 RuVector crates.
- **RVF clarification:** RVF is NOT eliminated. `rvf-runtime` stays as the agent experience persistence layer (Tier 3). It is only eliminated as *knowledge graph* runtime storage (Samyama RocksDB covers that). The `.rvf` file format stores: agent trajectories, SONA MicroLoRA weights, compacted agent memory, reward history — signed, COW-branched, portable.
- **Graph healing (§11.4):** Three layers — RuVector crates (`ruvector-hnsw-repair` for HNSW vector index, `ruvector-proof-gate` for write provenance), our Cypher-based knowledge graph health checks (orphan/dangling/duplicate/missing), and write-time prevention (schema validation + proof gate + VID canonicalization). RuVector heals the **vector index layer**; we heal the **knowledge graph layer**.
- **Observability (§12):** Health endpoints (da health), Prometheus-ready metrics (graph_nodes_total, evidence_resolvability_rate, llm_calls_total, import_eligible gauge), alerting thresholds (Samyama down=CRITICAL, resolvability<0.7=HIGH, import_eligible=1 without audit=CRITICAL), structured tracing per operation.
- **EvidenceStore (§13):** Port linking Samyama graph nodes (by VID) to immutable evidence artifacts (PDF hash, TEI, ODL layout, page/bbox spans). Every Entity/Relation must have HAS_EVIDENCE edge before import. verify_chain() called by REVIEW FSM state.

---

## 10. Optimization Stack (Rust-native, no Python DSPy required)

> DSPy/GRPO guard from ADR-039 rule #8 preserved: optimizers activate only after Stage 2+ metrics verified on real batches. This section documents what WILL be used when that gate opens. Python DSPy is NOT needed.

### 10.1 What RuVector SONA already provides (lightweight RL)

SONA is closer to GRPO than initially apparent. It implements a full REINFORCE policy-gradient loop with continual-learning safeguards:

| SONA component | File | DSPy/GRPO equivalent |
|---------------|------|--------------------|
| **REINFORCE** with baseline | `sona/src/types.rs` | Simplified GRPO (no KL penalty, no clipping) |
| **MicroLoRA** (rank 1-2, <100μs update) | `sona/src/lora.rs` | LoRA fine-tuning (GRPO uses rank 4-16; SONA uses 1-2 for CPU speed) |
| **EWC++** (Fisher matrix + regularization) | `sona/src/ewc.rs` | Catastrophic forgetting prevention (continual RL) |
| **Darwin guard** (strategy evolution) | `sona/src/darwin_guard.rs` | Multi-objective optimization (NSGA-II-style) |
| **Auto-tuner** (parameter optimization) | `sona/src/auto_tuner.rs` | Bayesian optimization (TPE-style in DSPy) |
| **ReasoningBank** (case memory) | `sona/src/reasoning_bank.rs` | Few-shot example bank (KNNFewShot-style) |
| **ruvltra_pretrain** (warm-start patterns) | `ruvllm/src/sona/ruvltra_pretrain.rs` | BootstrapFewShot + warm-start |

**Verdict:** SONA covers retrieval adaptation ("learn what retrieval works better") via REINFORCE + MicroLoRA. This is the in-process, sub-millisecond learning loop that runs on every query trajectory. It does NOT cover prompt text optimization or model weight fine-tuning.

### 10.2 External Rust crates (when Stage 2+ gate opens)

| Crate | Role | License | Maturity |
|-------|------|---------|----------|
| **`gepa`** (crates.io) | Genetic-Pareto prompt evolution. ICLR 2026 Oral. **+6% over GRPO**, **35x fewer rollouts**. Reflective mutation + Pareto-front selection. Provider-agnostic (works with 9router). `unsafe_code = "forbid"`. | MIT | v0.1.0, 1 contributor |
| **`dsrust`** (crates.io) | Faithful byte-for-byte DSPy port. 696 Rust tests, 452 DSPy tests passing. Includes MIPROv2, GEPA, COPRO, BootstrapFewShot. Cross-compatible with Python DSPy (same save/load format). | MIT/Apache-2.0 | v0.1.0-alpha.1 |
| **`productioneer/dspy-rs`** (GitHub) | Full-parity DSPy v3.1.3 port. ALL optimizers: LabeledFewShot, BootstrapFewShot, COPRO, MIPROv2, SIMBA, GEPA, **GRPO**, KNNFewShot, AvatarOptimizer, BetterTogether. Includes TPE + Proposer + Provider/TrainingJob/ReinforceJob. | — (GitHub) | active dev |
| **`dspy-rs`** krypticmouse (crates.io) | Ground-up DSPy rewrite with COPRO + MIPROv2. Builder pattern, derive macros. | — | v0.7.3, 39K downloads, beta |
| **`kkachi`** (crates.io) | Composable prompt pipelines, multi-objective Pareto tuning, zero-copy core. | AGPL (commercial avail) | v0.1.8 |

### 10.3 Samyama/RuVector metaheuristics (numerical optimization)

| Algorithm | Crate | Application |
|-----------|-------|------------|
| **NSGA-II** (fast non-dominated sort, crowding distance, elite archive) | `samyama-optimization/moo.rs` | Multi-objective: recall vs precision vs cost tradeoff for prompt candidates |
| **Hypervolume / IGD metrics** | `samyama-optimization/moo.rs` | Quality assessment of Pareto fronts |
| **Jaya, Rao, GWO, TLBO** | `ruvector-optimization` | Hyperparameter optimization (HNSW ef, BM25 k1/b, fusion weights) |

### 10.4 Optimization strategy per phase

| Phase | Optimizer | What it optimizes | Guard |
|-------|-----------|------------------|-------|
| **1-3** (foundation) | None — statistical-first only | — | ADR-039 rule #8: no optimizer until Stage 2+ metrics |
| **4** (agent) | **RuVector SONA** | Retrieval weights (MicroLoRA rank 1-2) | Already integrated; runs per-query trajectory |
| **4+** (prompt tuning) | **`gepa` crate** | Prompt instruction text (reflective mutation) | Activate after Stage 2 metrics verified; GT isolation enforced |
| **4+** (few-shot) | **`dsrust`** | Few-shot example selection (BootstrapFewShot) | Same guard; cross-compatible with Python DSPy if needed |
| **5+** (RL fine-tune) | **`productioneer/dspy-rs` GRPO** | Model weights (small model for extraction) | GPU required; Stage 3+; GT isolation; D127 import gate |
| **any** (numerical) | **samyama-optimization NSGA-II** | Hyperparameters (ef_search, k1, fusion weights) | No guard needed — numerical, not semantic |

### 10.5 Binding rule

> **Python DSPy is eliminated from the stack.** The full optimization toolkit exists in Rust: `gepa` for prompt evolution, RuVector SONA for retrieval learning, `dsrust`/`productioneer/dspy-rs` for DSPy parity including GRPO, and `samyama-optimization` for multi-objective numerical optimization. No Python dependency for any optimization task.

Guard (ADR-039 rule #8) remains: no optimizer activates before Stage 2+ metrics verified on real batches with GT isolation enforced.
