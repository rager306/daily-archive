# Reliability Analysis: Embedded Store Data Persistence

## Problem

`SamyamaGraphStore` uses `Arc<RwLock<GraphStore>>` — in-process memory. Each `da ingest` CLI invocation creates a **fresh** GraphStore. Data is lost on process exit. Batch of 10 papers → 10 separate processes → 10 separate stores → 0 nodes survive.

## Samyama Persistence Mechanisms (3 layers)

### Layer 1: RocksDB PersistenceManager

```rust
// src/persistence/mod.rs
PersistenceManager::new("./samyama_data")
  → opens RocksDB at ./samyama_data/data/
  → opens WAL at ./samyama_data/wal/
  → creates tenant column families
```

**Used by:** Samyama **server mode** only (`RespServer`). Server initializes `PersistenceManager`, recovers persisted data on startup, writes through WAL+RocksDB on every mutation.

**NOT used by:** `GraphStore::new()` (bare in-memory). Our `EmbeddedClient` creates bare `GraphStore::new()` — no persistence.

### Layer 2: Snapshot (.sgsnap)

```rust
// src/snapshot/persist.rs
persist_snapshot(data_path, bytes)  // atomic write: tmp → fsync → rename → .committed
restore_persisted_snapshots(data_path, &mut graph)  // replay on boot
```

**Used by:** Server mode — on startup, if no RocksDB recovery, replays last `.sgsnap` from `snapshots/` directory. Also used for bulk import (`import_tenant` / `import_tenant_with_dedup`).

**Flow:** `export_tenant(store, writer)` → gzip JSON-lines → `persist_snapshot(path, bytes)`.

### Layer 3: WAL (Write-Ahead Log)

```rust
// src/persistence/wal.rs
Wal::new("./samyama_data/wal")
  → append-only log with CRC32 checksums
  → sequence numbers for ordering
  → checkpoint/truncate after RocksDB flush
```

**Used by:** `PersistenceManager` internally. Every mutation appends to WAL before in-memory state changes. On crash recovery, WAL replayed.

## Three Solutions (ranked)

### Solution A: Long-running daemon process (RECOMMENDED)

**Architecture:** Samyama server runs as a persistent background process. `da` CLI connects via HTTP/RESP, NOT embedded.

```
┌─────────────────────────────────────────────┐
│  Samyama Server (persistent daemon)          │
│  RESP :6380  +  HTTP :8080                   │
│  RocksDB + WAL + snapshots                   │
│  Data survives restarts ✅                    │
└──────────────────┬──────────────────────────┘
                   │ HTTP / RESP
┌──────────────────▼──────────────────────────┐
│  da CLI / da-agent / da-server              │
│  RemoteClient (HTTP) for all operations     │
│  COLD path only (~5-15ms per query)         │
└─────────────────────────────────────────────┘
```

**Pros:** Data persists. Standard deployment pattern. Server handles recovery, WAL, snapshots.
**Cons:** HTTP overhead (~5-15ms per query vs <1ms embedded). Loses HOT path advantage.
**When:** Production, multi-process, multi-user.

### Solution B: Single-process batch daemon

**Architecture:** `da batch-ingest` is a single long-running process that creates the embedded store, ingests all papers, then exports snapshot.

```rust
// da batch-ingest --pdfs-dir <dir> --output snapshot.sgsnap
async fn batch_ingest(pdfs: Vec<PathBuf>) {
    let store = SamyamaGraphStore::new();  // fresh embedded
    for pdf in pdfs {
        ingest_pdf(&store, &pdf).await;    // HOT path, <1ms graph write
    }
    // Export snapshot for durability
    let snapshot = store.export_snapshot().await?;
    std::fs::write("data/samyama/snapshot.sgsnap", &snapshot)?;
    println!("Exported {} nodes to snapshot", store.node_count().await);
}
```

**Pros:** HOT path (direct API, <1ms). Snapshot persists data. Single process = no concurrency issues.
**Cons:** No live queries during ingest. Snapshot is full-graph export (not incremental).
**When:** Batch ETL, nightly ingest, development.

### Solution C: Embedded with PersistenceManager

**Architecture:** EmbeddedClient wraps `PersistenceManager` directly, not just bare `GraphStore`.

```rust
pub struct SamyamaGraphStore {
    store: Arc<RwLock<GraphStore>>,
    engine: QueryEngine,
    persistence: Option<Arc<PersistenceManager>>,  // NEW
    tenant: String,
}

impl SamyamaGraphStore {
    pub fn with_persistence(data_path: &str) -> Self {
        let pm = PersistenceManager::new(data_path)
            .expect("persistence init");
        let store = Arc::new(RwLock::new(GraphStore::new()));
        // Recover from RocksDB
        // ... replay WAL
        Self { store, persistence: Some(Arc::new(pm)), ... }
    }

    pub async fn create_node_direct(&self, label: &str) -> NodeId {
        let mut store = self.store_write().await;
        let id = store.create_node(Label::new(label));
        // Persist through WAL + RocksDB
        if let Some(ref pm) = self.persistence {
            pm.persist_node(&self.tenant, &store.get_node(id).unwrap());
        }
        id
    }
}
```

**Pros:** HOT path + persistence. Data survives restarts. WAL replay on startup.
**Cons:** Complex — need to wire PersistenceManager into every direct API call. Samyama's server already does this, but the SDK EmbeddedClient doesn't expose it.
**When:** When we need both speed AND persistence in a single process.

## Recommendation

**Phase 2 (now):** Solution B — `da batch-ingest` single process + snapshot export. Proves HOT path + gives durability.

**Phase 3+ (production):** Solution A — Samyama server daemon + RemoteClient. Standard deployment, handles recovery/WAL/snapshots automatically. Accept HTTP overhead for durability.

**Phase 5 (optimization):** Solution C — if HOT path is critical for production throughput, wire PersistenceManager into EmbeddedClient. But this is premature optimization until we measure actual bottleneck.

## Implementation Plan

### Step 1: `da batch-ingest` command (Solution B)

```bash
# Single process, HOT path, snapshot export
da batch-ingest --pdfs-dir data/article_catalog --output data/samyama/batch.sgsnap

# Or explicit list
da batch-ingest --pdf paper1.pdf --id 1206.6423 --pdf paper2.pdf --id 1606.01540 --output snapshot.sgsnap
```

### Step 2: `da load-snapshot` command

```bash
# Load snapshot into Samyama server
da load-snapshot --input data/samyama/batch.sgsnap

# Or into embedded (for testing)
da load-snapshot --input batch.sgsnap --embedded
```

### Step 3: Samyama server as systemd service (Solution A)

```bash
# Start Samyama server with persistence
samyama --port 6380 --http-port 8080 --data-path data/samyama

# da CLI uses HTTP (RemoteClient)
da ingest --pdf paper.pdf --id 1206.6423 --remote
```

## Why NOT just use server mode now?

1. **Current da-adapters SamyamaGraphStore uses EmbeddedClient** — no HTTP overhead
2. **HOT path is 15x faster** — critical for batch ingest of 60+ papers
3. **Server mode loses HOT path** — everything goes through Cypher parse+plan+HTTP
4. **Development iteration is faster** with embedded — no server to start/stop
5. **Phase 2 goal:** prove the pipeline works, not optimize deployment

Switch to server mode when:
- Multiple processes need to access the same graph
- Data must persist between CLI invocations
- Agent layer needs live graph access during ingest

## Round-trip verification (2026-07-26)

Tested snapshot export → import round-trip with `da` CLI:

```
$ da batch-ingest --ids 1206.6423,1606.01540 --output /tmp/test.sgsnap
   Nodes in graph: 2
   Snapshot: /tmp/test.sgsnap (536 bytes)

$ da graph-stats          # fresh process
  Nodes: 0                # ← in-memory store reset (expected)

$ da load-snapshot --input /tmp/test.sgsnap
✅ Snapshot loaded — 2 nodes now in graph   # ← import works in-process

$ da graph-stats          # fresh process again
  Nodes: 0                # ← data not persisted across processes
```

**Conclusion:** Snapshot round-trip works (export → import → nodes visible
in same process). But cross-process durability still requires Solution A
(Samyama server with RocksDB) or Solution C (embedded PersistenceManager).
Snapshot is a **backup/restore** mechanism, not live persistence.

### Current CLI commands

| Command | Purpose | Persistence |
|---------|---------|:-----------:|
| `da batch-ingest --ids ... --output f.sgsnap` | Ingest + export snapshot | export only |
| `da load-snapshot --input f.sgsnap` | Restore snapshot into process | in-process |
| `da graph-stats` | Show node/edge counts | read-only |
| `da health` | Infrastructure check | read-only |
