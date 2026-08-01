# daily-archive v2

Local-first **scientific knowledge engine**: ingest PDFs + HTML → extract
structure → embed → write to a durable knowledge graph (Samyama) → agent
retrieval. Graph import is **fail-closed** (`import_eligible=false`) until
explicit human go (D127).

**Runtime:** Rust workspace under `crates/` (hexagonal / onion).  
**Frozen evidence:** Python stack under `legacy/` (M274–M284, Wave B, hybrid bodies).  
**Binding ADRs:** `doc/adr/ADR-INDEX.md` (037–042).

---

## Current state (2026-07-29)

| Area | Status |
|------|--------|
| Phase 1 domain + ports | Done (`da-domain`, `da-ports`) |
| Phase 2 ingest (HOT path) | Done — GROBID → embed → direct GraphStore write |
| Phase 2 multi-source | **Done** — HtmlParser for GNN textbook + Source node (Layer 0) |
| Section node creation | **Done** — Paper → hasPart → Section (Layer 2 materialized) |
| Extraction corpus | **104 sources** (100 arxiv PDF + 4 GNN textbook HTML), 746 gold |
| Extraction metrics | **P=0.759 R=0.999 F1=0.862** (1 FN: `generalization`) |
| Cross-domain entities | **Done** — GCN, GAT, GraphSAGE, GNN in KNOWN_METHODS |
| Declarative patterns | **Done** — `data/extraction_patterns.json` (Wave 3) |
| Governor CLI tools | **Done** — eval_batch pre-commit hook, suggest-whitelist, eval_html |
| Phase 3 edge weights | **Done** — `set_edge_property_float` in DirectGraphStore |
| Phase 3 entity embeddings | **Done** — EntitySchema embedding field + ExtractionUseCase.with_embedder() |
| Phase 3 section embeddings | **Done** — SectionSchema embedding field |
| Phase 4 hypergraph | **Done** — ConceptCluster node + MEMBER_OF edges + detect_clusters() |
| Phase 5 GNN algorithm ports | **Done** — GraphAlgorithms trait (PPR, get_neighbors, get_all_neighbors) |
| Graph schema | **Done** — 29 node types, 26 indexes, `da schema init` |
| OpenAlex enrichment | **Done** — `da enrich` fetches topics, authors from OpenAlex API |
| Import / graph write | **Locked** (`import_eligible=false`, D127) |
| Cross-process live graph | **Not yet** — needs Samyama server mode (Phase 3+) |
| RuVector GNN (Tier 2) | **Port traits ready** (9/10 GNN readiness), adapter pending |
| Agent layer (RuVector/SONA) | Vendored, port traits defined |

Architecture:

```
crates/
  da-domain/        # pure types: Paper, Entity, Evidence, VID, schema,
                    #   Source, ConceptCluster, hypergraph, cluster detection
  da-ports/         # traits: GraphStore, DirectGraphStore, ParserPort,
                    #   Embedder, Extractor, OpenAlexClient, LLMClient,
                    #   GraphAlgorithms (PPR, neighbors)
  da-application/   # use cases: Ingest, BatchIngest, Extraction (with embedder),
                    #   Enrich, Healing, Scheduler
  da-graph/         # Cypher query builders + schema DDL (26 indexes)
  da-adapters/      # GROBID parser, HtmlParser, FdApiEmbedder,
                    #   SamyamaGraphStore, RuleBasedExtractor, OpenAlexHttpAdapter
  da-cli/           # binary `da` + examples (eval_batch, eval_extract,
                    #   eval_html, suggest_whitelist)
```

Dependency direction (enforced by CI):

```
da-domain  ──►  (no infra)
da-ports   ──►  da-domain
da-application / da-graph  ──►  da-domain, da-ports  (no da-adapters)
da-adapters  ──►  da-domain, da-ports, samyama
da-cli       ──►  all of the above
```

---

## Quick start

```bash
# Build
cargo build -p da-cli

# Health (GROBID :8070, fd_api embedder, Samyama embedded)
./target/debug/da health

# Single paper (HOT path, in-memory — data lost on exit)
./target/debug/da ingest --pdf path/to/paper.pdf --id 1206.6423

# Batch + snapshot (durable backup)
./target/debug/da batch-ingest \
  --ids 1206.6423,1606.01540,1612.00341 \
  --output data/samyama/batch.sgsnap

# Extract entities from a paper
./target/debug/da extract --id 2507.19457

# Restore snapshot into this process
./target/debug/da load-snapshot --input data/samyama/batch.sgsnap
./target/debug/da graph-stats
```

### Sidecars

```bash
# GROBID CRF
docker compose -f .docker/docker-compose.yml --env-file .env up -d grobid
curl -sS http://127.0.0.1:8070/api/isalive   # true

# Embedder: fd_api on :8000 with FD_API_KEY in .env
# Env template: .env.example (GROBID_*, FD_API_*, SAMYAMA_*)
```

---

## Extraction evaluation

```bash
# Batch eval on 104 sources (100 arxiv PDF + 4 GNN textbook HTML)
cargo run -p da-cli --example eval_batch

# Single-paper extraction debug
cargo run -p da-cli --example eval_extract -- 2507.19457

# HTML source extraction (GNN textbook)
cargo run -p da-cli --example eval_html -- \
  data/article_catalog/article_catalog/gnn_textbook/html/gnn-ch-chapters__01-intro-to-graphs/source/chapter.html \
  gnn-ch-01

# Whitelist coverage analysis
cargo run -p da-cli --example suggest_whitelist
```

Current metrics: **P=0.759 R=0.999 F1=0.862** (104 sources, 746 gold entities).

---

## Development

```bash
# Format + compile + hexagonal check + unit tests
bash scripts/verify_rust_architecture.sh

# Or piece-wise (preferred — avoids rocksdb test-profile rebuild):
cargo fmt -p da-domain -p da-ports -p da-application -p da-graph -p da-adapters -p da-cli
cargo check --workspace
cargo test -p da-domain --lib
cargo test -p da-graph --lib
cargo test -p da-application --tests
cargo test -p da-adapters --lib

# Pre-commit (cargo fmt + cargo check + cargo clippy + eval_batch)
pre-commit install
pre-commit run --all-files
```

> **Gotcha:** `cargo test --workspace` is slow, not hung. Samyama's
> `librocksdb-sys` rebuilds for the test profile (multiple fingerprints,
> minutes each). Prefer per-package `cargo test -p …`.

---

## CLI commands

| Command | Purpose |
|---------|---------|
| `da health` | GROBID / embedder / Samyama health |
| `da version` | Version + ADR pin |
| `da ingest --pdf … --id …` | Single-paper HOT path |
| `da batch-ingest --ids a,b --output f.sgsnap` | Multi-paper + snapshot export |
| `da load-snapshot --input f.sgsnap` | Restore snapshot (same process) |
| `da graph-stats` | Node/edge counts |
| `da schema-init` | Initialize all 26 graph indexes |
| `da schema-check` | Audit create_node sites vs schema registry (ADR-045) |
| `da edge-contracts` | Print edge endpoint contract matrix (ADR-045 Wave G) |
| `da schema-list` | Print all registered node schemas as markdown (ADR-040) |
| `da extract --id <arxiv_id>` | Extract entities from paper |
| `da heal --op silence` | Silence a node (D135) |
| `da heal --op correct` | Correct a property (D135) |
| `da heal --op merge` | Merge duplicates (D135) |
| `da enrich --id <arxiv_id>` | Fetch metadata from OpenAlex (topics, authors) |
| `da batch-enrich --ids a,b,c` | Batch OpenAlex enrichment |
| `da scheduler run` | Process pending OpenAlex retry tasks (graph-persisted) |
| `da query --kind count` | Count Paper nodes (Cypher via da-graph) |
| `da query --kind by-arxiv --id ID` | Find paper by arxiv_id |
| `da query --kind citation-hops --id VID --hops 2` | K-hop citation neighborhood |
| `da query --kind orphans` | Find orphan nodes (no edges) |

---

## GNN readiness (9/10)

| Requirement | Status |
|-------------|--------|
| Embeddings on Work nodes | ✅ bge-m3 1024d |
| Embeddings on Entity nodes | ✅ EntitySchema + with_embedder() |
| Embeddings on Section nodes | ✅ SectionSchema |
| Edge weights | ✅ set_edge_property_float |
| Typed adjacency export | ✅ GraphAlgorithms::get_neighbors |
| Heterogeneous node types | ✅ 29 types |
| retrieval_eligible filter | ✅ D134 on ALL nodes |
| PPR from any node | ✅ GraphAlgorithms::personalized_pagerank |
| Community detection | ⏳ detect_clusters() offline; RuVector online pending |
| Agent assertions | ❌ Layer 7 future |

---

## Persistence model (ADR-041)

| Solution | Mode | Persistence | Status |
|----------|------|:-----------:|--------|
| **B: Batch + snapshot** | Embedded HOT path | `.sgsnap` export | **Current** (Phase 2) |
| **A: Server daemon** | RemoteClient / RESP | RocksDB + WAL | Phase 3+ |

---

## Layout

```text
crates/                 # Rust workspace (runtime)
legacy/                 # Frozen Python research_graph + tests + scripts
doc/adr/                # Binding ADRs (037–042 + INDEX)
doc/ONTOLOGY-DESIGN.md  # 7-layer ontology design
doc/GRAPH-SCHEMA.md     # Graph schema (29 nodes, 26 indexes)
data/article_catalog/   # Canonical PDFs + HTML chapters
data/gold_standard/     # 104 gold-standard fixtures
data/extraction_patterns.json  # Declarative extraction config
data/samyama/           # Snapshots (.sgsnap)
.docker/                # GROBID compose
scripts/                # verify_rust_architecture.sh
```

---

## Further reading

| Doc | Purpose |
|-----|---------|
| `doc/adr/ADR-INDEX.md` | Binding ADRs + supersession chain |
| `doc/adr/ADR-042-…` | HyCE-RAG hypergraph evidence chain model |
| `doc/ONTOLOGY-DESIGN.md` | 7-layer ontology (L0-L7) |
| `doc/GRAPH-SCHEMA.md` | 29 node types, 26 indexes, edge weights |
| `doc/PERSISTENCE-ANALYSIS.md` | Why snapshot vs server |
| `CHANGELOG.md` | Recent changes |
