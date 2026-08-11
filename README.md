# daily-archive v2

Local-first **scientific knowledge engine**: ingest PDFs + HTML → extract
structure → embed → write to a durable knowledge graph (Samyama) → agent
retrieval. Graph import is **fail-closed** (`import_eligible=false`) until
explicit human go (D127).

**Runtime:** Rust workspace under `crates/` (hexagonal / onion).
**Frozen evidence:** Python stack cold-archived in `archive/python-era-cold-*/` (`archive/PYTHON-ERA-COLD.md`); `legacy/` stub only.
**Binding ADRs:** `doc/adr/ADR-INDEX.md` (037–042).

---

## Current state (2026-08-05)

| Area | Status |
|------|--------|
| Phase 1 domain + ports | Done (`da-domain`, `da-ports`) |
| **kg-* universal crates** | **Done (ADR-050)** — kg-ontology, kg-storage, kg-embeddings, kg-algorithms, kg-pipeline |
| **ADR-051 P0 contract suites** | **Done** — GraphStore + Embedder contracts; export/import round-trip (M306); serde roundtrip 40 types / 53 tests (M340–M343) |
| **ADR-051 §8.1 graph task suite** | **Done (M336–M338)** — `graph_task_suite` runner; `@expect_rows` / `@expect_min_value` bounds |
| **ADR-051 P1 proptest** | **Started (M344/M353–M355)** — 33 proptest: TemporalEdge, EntityType/RelationType, VID, confidence, graph_tasks |
| **PDF OCR triage** | **Done (M346–M351)** — `pdf-inspector` pre-GROBID classifier (feature `pdf-classifier`); hub pages 69/84 = `suspected_garbled_text` |
| Phase 2 ingest (HOT path) | Done — GROBID → embed → direct GraphStore write |
| Graph schema | **Done** — 31 node types, 26 indexes, `da schema init` |
| **Governor CLI** | **Done** — YAML rules, trajectory, dead-code scan |
| OpenAlex enrichment | **Done** — `da enrich` |
| Import / graph write | **Locked** (`import_eligible=false`, D127) |
| **Wave 3 health path** | **Done (M307–M310)** — `da serve-graph` (in-memory RESP, optional `--snapshot`), `da graph-ping` |
| Cross-process GraphStore | **Scalar read plane in progress (M378)** — bounded RESP health/query adapter; vectors/index/snapshot/HOT writes explicitly unsupported |
| RuVector GNN (Tier 2) | Port traits ready; adapter pending |
| Agent layer (RuVector/SONA) | Vendored, port traits defined |

Architecture:

```
crates/
  da-domain/        # pure types: Paper, Entity, Evidence, VID, schema,
                    #   Source, ConceptCluster, hypergraph, cluster detection
  da-ports/         # traits: GraphStore, DirectGraphStore, ParserPort,
                    #   Embedder, Extractor, OpenAlexClient, LLMClient,
                    #   GraphAlgorithms (PPR, neighbors)
  da-application/   # use cases: Ingest, Extraction, Enrich, Healing, Scheduler
  da-adapters/      # GROBID/HTML, FD API, Samyama, extraction/OpenAlex adapters
  da-cli/           # binary `da` + cache/materialization/evaluation operators
  da-mcp/           # read-only MCP knowledge-base access
  kg-ontology/      # universal YAML-driven ontology registry and temporal types
  kg-storage/       # universal graph storage ports + Samyama adapter
  kg-embeddings/    # universal embedding port + FD API adapter
  kg-algorithms/    # universal graph algorithms
  kg-pipeline/      # cacheable pure pipeline contracts
```

Dependency direction (enforced by CI):

```
da-domain  ──►  (no infra)
da-ports   ──►  da-domain
da-application ──►  da-domain, da-ports  (no da-adapters)
da-adapters  ──►  da-domain, da-ports, samyama
da-cli / da-mcp ──► composition and infrastructure layers
kg-*          ──►  universal only (never da-*)
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

# Universal load plane (ADR-056): resolve → ensure catalog → optional ingest/extract
# Network fail-closed; D127 import_eligible stays false unless --enable-import
./target/debug/da load 'https://arxiv.org/abs/2608.07067' --allow-network --stages ensure
./target/debug/da load 'https://arxiv.org/abs/2608.07067' --allow-network --stages ingest+extract \
  --output artifacts/article_context/paper_2608.07067.sgsnap \
  --report artifacts/loads/2608.07067.json
# Exact arXiv revision: canonical Paper id stays versionless, source bytes/cache do not.
# ADR-058 accepted: production ontology includes PaperRevision and
# SourceManifestation DATA. Versioned load materializes revision-scoped Sections,
# embeddings, and FOUND_IN provenance while canonical Paper identity stays stable.
./target/debug/da load 'https://arxiv.org/html/2506.07853v1' --allow-network \
  --stages ingest+extract --cache-root artifacts/pipeline_cache/2506.07853v1 \
  --output artifacts/article_context/paper_2506.07853v1.sgsnap
# Optional TeX source sidecar (+ inventory) under catalog:
./target/debug/da load 2608.07067 --allow-network --stages ensure --include-tex-source

# ADR-058 real-store acceptance: one Paper, v1+v5 revisions, replay + snapshot
cargo run -p da-cli --example revision_graph_canary
cargo run -p da-cli -- graph-stats --snapshot \
  artifacts/article_context/revision_graph_2506.07853.sgsnap
# Read-only prepared revision query: current/history/exact vN plus grounded
# summaries and current readiness state/reasons/policy lineage in JSON.
cargo run -p da-cli -- revision-query --id 2506.07853 --selector history \
  --snapshot artifacts/article_context/revision_graph_2506.07853.sgsnap --json
# Deterministic revision summaries: generic Summary nodes, DATA-driven semantic
# role (`methods`, `results`, or `generic`), exact Section VID and policy SHA.
# The real v1/v5 fixture has Methods evidence but no Results section; absence is
# reported honestly rather than inferred or generated by an LLM.
cargo run -p da-cli --example extractive_summary_canary
# Revision-scoped deterministic readiness derived from exact Sections,
# manifestations and grounded summaries. Policy thresholds/expected roles are
# ontology DATA; result is ready|partial|blocked with explicit reasons.
cargo run -p da-cli --example revision_readiness_canary -- \
  --snapshot artifacts/article_context/revision_graph_2506.07853_with_summaries.sgsnap

# GNN textbook: DATA-driven full catalog ensure / content hub
./target/debug/da load --batch-toc data/source_catalogs/gnn_textbook_toc.yaml --allow-network
./target/debug/da load --batch-toc data/source_catalogs/gnn_textbook_toc.yaml \
  --stages ingest --cache-root artifacts/pipeline_cache/gnn_textbook \
  --batch-report-dir artifacts/loads/gnn_textbook \
  --output artifacts/article_context/gnn_textbook_hub.sgsnap
# Exact replay: the same command with local catalog + warm cache needs no network
# and no FD_API_KEY; every report records parse=hit + embed=hit.

# Batch + snapshot (durable backup) — legacy multi-id path still available
./target/debug/da batch-ingest \
  --ids 1206.6423,1606.01540,1612.00341 \
  --output data/samyama/batch.sgsnap

# Extract entities from a paper
./target/debug/da extract --id 2507.19457

# Restore snapshot into this process (still dies when process exits)
./target/debug/da load-snapshot --input data/samyama/batch.sgsnap
./target/debug/da graph-stats

# Wave 3 multi-process HEALTH (in-memory RespServer — no RocksDB)
# Terminal A: serve (optional --snapshot to preload .sgsnap into this process)
./target/debug/da serve-graph --host 127.0.0.1 --port 16379
# or: da serve-graph --snapshot data/samyama/batch.sgsnap --port 16379
# Terminal B: health probe
./target/debug/da graph-ping --port 16379   # → PONG
# Direct universal read-plane probe: scalar GraphStore query + fail-closed write
cargo run -p kg-storage --example remote_graph_probe -- 127.0.0.1:16379
# Product CLI composition (JSON stdout only; no daemon auto-start).
# Both embedded and remote modes share one fail-closed read-only Cypher policy:
cargo run -p da-cli -- graph-query --mode remote --endpoint 127.0.0.1:16379 \
  --cypher 'MATCH (r:PaperRevision) RETURN r.revision AS revision' --json
# Mode-aware operator health: probes scalar reads while keeping HOT writes explicit.
DA_GRAPH_MODE=remote DA_GRAPH_ENDPOINT=127.0.0.1:16379 \
  cargo run -p da-cli -- health
# Note: default port 16379 (not Redis 6379, not vendor SAMYAMA_RESP_PORT 6380)
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
# Batch eval on all 200 reviewed gold fixtures (PDF/HTML)
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

Current complete-denominator metrics: **P=0.766 R=1.000 micro-F1=0.868**
(200/200 fixtures, 1,439 gold entities; see
`artifacts/gold_corpus/eval_aggregate_all_sources.json`).

---

## Development

```bash
# Format + compile + hexagonal check + unit tests
bash scripts/verify_rust_architecture.sh

# Or piece-wise (preferred — avoids rocksdb test-profile rebuild):
cargo fmt -p da-domain -p da-ports -p da-application -p da-adapters -p da-cli -p da-mcp
cargo check -p da-cli
cargo test -p da-domain --lib
cargo test -p kg-ontology --lib
cargo test -p da-application --tests
cargo test -p da-adapters --lib

# ADR-051 P1 property tests (M344/M353–M355)
cargo test -p kg-ontology --lib proptest   # TemporalEdge (8)
cargo test -p da-domain --lib proptest     # EntityType/RelationType/VID/confidence (19)
cargo test -p da-cli --lib proptest        # graph_tasks parser (6)

# ADR-051 §8.1 semantic graph task suite (M336–M338)
cargo run -p da-cli --example graph_task_suite -- \
  --snapshot artifacts/gepa_hub_production/hub.sgsnap \
  --tasks data/graph_tasks/gepa_hub.tasks.cypher

# Detect-only ontology v2 contract + identity compatibility audit (M371/S06)
cargo run -p da-cli --example ontology_contract_audit -- \
  --snapshot artifacts/article_context/materialization_all_200.sgsnap \
  --report artifacts/ontology/ontology_v2_contract_audit.json \
  --identity-report artifacts/ontology/ontology_v2_identity_compatibility.json

# Bounded real-Samyama ontology endpoint journey (M371/S06)
cargo run -p da-cli --example ontology_endpoint_canary -- \
  --report artifacts/ontology/ontology_v2_endpoint_canary.json

# Bind checkpoint + endpoint evidence without merging their graph identity
cargo run -p da-cli --example ontology_v2_convergence_canary -- \
  --report artifacts/ontology/ontology_v2_convergence_canary.json

# PDF OCR triage via pdf-inspector (M346–M351; optional feature)
cargo test -p da-adapters --features pdf-classifier --lib pdf_classifier
# cargo build -p da-cli --features pdf-classifier  # CLI with pre-GROBID triage

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
| `da load <locator> [--allow-network] [--stages ensure\|ingest\|ingest+extract] [--include-tex-source] [--cache-root …] [--report …] [--output …]` | Universal acquisition+load plane: parse+embed+extract cache; report carries physical snapshot SHA plus strict VID-based semantic SHA; D127 locked |
| `da load --batch-toc data/source_catalogs/….yaml [--stages …] [--cache-root …] [--batch-report-dir …] [--output hub.sgsnap]` | DATA-driven batch ensure or hub content; atomic per-unit reports and one process-local snapshot |
| `da load-snapshot --input f.sgsnap` | Restore snapshot (same process; non-persistent) |
| `da serve-graph [--snapshot f.sgsnap] [--port 16379]` | In-memory RESP server (M308/M309; no RocksDB; D127 locked) |
| `da graph-ping [--port 16379]` | RESP PING → PONG against serve-graph; M378 scalar remote query adapter is separate from HOT writes |
| `da graph-stats` | Node/edge counts |
| `da analyze --algorithm ppr --seed …` | Graph algorithms on process-local store (Wave 1–2) |
| `da schema-init` | Initialize all 26 graph indexes |
| `da schema-check` | Audit create_node sites vs schema registry (ADR-045) |
| `da edge-contracts` | Print edge endpoint contract matrix (ADR-045 Wave G) |
| `da schema-list` | Print all registered node schemas as markdown (ADR-040) |
| `da cross-refs` | Print cross-reference field registry (ADR-045 Wave F) |
| `da validate-node --label X` | Validate a node snapshot from stdin JSON (ADR-045 Wave D CLI) |
| `da audit-fields` | Audit pipeline-set vs schema-declared fields (ADR-045 / MEM508) |
| `da validate-graph [--label X]` | Validate the live graph against schema registry (ADR-045 Wave D production) |
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

## Next gate (trajectory enforcement)

Architecture is crystallized on the Rust onion (ADR-040 Samyama +
ADR-050 kg-* universal crates). Hexagonal guardrail
(scripts/verify_rust_architecture.sh) enforces da-domain <- da-ports <-
da-application <- da-adapters/da-cli direction and ADR-050 invariant
(kg-* must not depend on da-*). Governor CLI
(scripts/governor/) provides trajectory enforcement with 15 ADR rules
in YAML data files.

- **Phase 1 ✅**: Rust workspace migration (da-* crates), SamyamaGraphStore (ADR-040)
- **Phase 2 ✅**: Extraction eval corpus (12 modern papers, F1=0.774 micro)
- **Phase 3 (active)**: kg-* crate family (ADR-050) — Phase A+D done, Phase B follow-up pending
- **Phase 3.5 ✅**: ADR-051 P0 contract suites — GraphStore + Embedder contract suites (Mock + Samyama), serde roundtrip tests (M291)
- **Phase 4 (active)**: OntologyRegistry reads YAML as source of truth; the bounded detect-only contract audit currently reports explicit endpoint/topology/consumer coverage gaps instead of semantic PASS (ADR-050 Phase D/F, M371)
- **Phase 5 (active)**: identity/property policies and accepted VID bytes are stable; Citation/QualitySignal materializers, real endpoint tasks, injected finalize and digest-bound split-source convergence evidence are executable. The bounded report remains honestly `incomplete`; a supported acceptance runner and absent semantic layers remain (NodeSchemaDef -> OntologyRegistry)

Import lock (D127): `import_eligible = false`. Graph writes stay locked
until explicit user yes.

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
