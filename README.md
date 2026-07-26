# daily-archive v2

Local-first **scientific knowledge engine**: ingest PDFs → extract structure →
embed → write to a durable knowledge graph (Samyama) → agent retrieval.
Graph import is **fail-closed** (`import_eligible=false`) until explicit human go (D127).

**Runtime:** Rust workspace under `crates/` (hexagonal / onion).  
**Frozen evidence:** Python stack under `legacy/` (M274–M284, Wave B, hybrid bodies).  
**Binding ADRs:** `doc/adr/ADR-INDEX.md` (037–041).

---

## Current state (2026-07-26)

| Area | Status |
|------|--------|
| Phase 1 domain + ports | Done (`da-domain`, `da-ports`) |
| Phase 2 ingest (HOT path) | Done — GROBID → embed → direct GraphStore write |
| Batch 10-paper canary | **10/10 ok**, ~39s, snapshot exported |
| Section/citation parsing | Done — TEI sections + citations extracted + persisted |
| Citation graph (CITES) | Done — Citation nodes + CITES edges for resolvable refs |
| Snapshot durability | Export + load-snapshot round-trip verified (in-process) |
| Cross-process live graph | **Not yet** — needs Samyama server mode (Solution A, Phase 3+) |
| Import / graph write | **Locked** (`import_eligible=false`, D127) |
| Phase 3 extraction (GLiNER) | Not started |
| Phase 3 rule-based extraction | **Started** — Extractor port + RuleBasedExtractor + ExtractionUseCase |
| Agent layer (RuVector/SONA) | Vendored, not wired |

Architecture:

```
crates/
  da-domain/        # pure types: Paper, Entity, Evidence, VID, schema
  da-ports/         # traits: GraphStore, DirectGraphStore, ParserPort, Embedder, …
  da-application/   # use cases: IngestUseCase, batch_ingest_pdfs
  da-graph/         # Cypher query builders + schema DDL (no Samyama SDK)
  da-adapters/      # GROBID, FdApiEmbedder, SamyamaGraphStore
  da-cli/           # binary `da`
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

# Pre-commit (cargo fmt + cargo check)
pre-commit install
pre-commit run --all-files
```

> **Gotcha:** `cargo test --workspace` is slow, not hung. Samyama’s
> `librocksdb-sys` rebuilds for the test profile (multiple fingerprints,
> minutes each). Prefer per-package `cargo test -p …`. Concurrent cargo
> processes block on `target/` lock — cancel background cargo jobs first.

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
| `da query --kind count` | Count Paper nodes (Cypher via da-graph) |
| `da query --kind by-arxiv --id ID` | Find paper by arxiv_id |
| `da query --kind citation-hops --id VID --hops 2` | K-hop citation neighborhood |
| `da query --kind orphans` | Find orphan nodes (no edges) |

---

## Persistence model (ADR-041)

| Solution | Mode | Persistence | Status |
|----------|------|:-----------:|--------|
| **B: Batch + snapshot** | Embedded HOT path | `.sgsnap` export | **Current** (Phase 2) |
| **A: Server daemon** | RemoteClient / RESP | RocksDB + WAL | Phase 3+ |
| **C: Embedded + PersistenceManager** | Embedded HOT | RocksDB + WAL | Phase 5 optional |

Snapshot is a **backup/restore** mechanism, not live multi-process store.
Details: `doc/PERSISTENCE-ANALYSIS.md`.

---

## Architecture guardrail (CI)

`.github/workflows/architecture-guardrail.yml` enforces:

1. `cargo fmt` (our crates only)
2. `cargo check --workspace`
3. `cargo clippy -p da-* -- -D warnings`
4. Hexagonal dependency direction (no infra in domain/ports; no adapters in application/graph)

Local mirror: `scripts/verify_rust_architecture.sh`.

---

## Layout

```text
crates/                 # Rust workspace (runtime)
legacy/                 # Frozen Python research_graph + tests + scripts
doc/adr/                # Binding ADRs (037–041 + INDEX)
doc/PERSISTENCE-ANALYSIS.md
data/article_catalog/   # Canonical PDFs
data/samyama/           # Snapshots (.sgsnap)
.docker/                # GROBID / ODL compose
scripts/                # verify_rust_architecture.sh
artifacts/              # Historical Python evidence (ETL, Wave B)
archive/                # Rename shims only (not runtime)
```

---

## Intentionally deferred

| Item | Status |
|------|--------|
| Production graph import | Locked without human go (D127) |
| Cross-process live graph | Needs Samyama server (Solution A) |
| GLiNER 2 extraction | Phase 3 |
| RuVector agent brain (SONA/GNN) | Vendored, not wired |
| DSPy / optimizers | Guarded until Stage 2+ metrics |

---

## Further reading

| Doc | Purpose |
|-----|---------|
| `doc/adr/ADR-INDEX.md` | Binding ADRs + supersession chain |
| `doc/adr/ADR-040-…` | Tech stack lock (Samyama + RuVector + RVF) |
| `doc/adr/ADR-041-…` | Embedded Cypher + HOT path + access tiers |
| `doc/PERSISTENCE-ANALYSIS.md` | Why snapshot vs server |
| `doc/REPO-HYGIENE.md` | Garbage policy |
| `CHANGELOG.md` | Recent changes |
| `legacy/README.md` (if present) | Frozen Python evidence path |
| `.agents/skills/samyama/SKILL.md` | Samyama integration skill |
