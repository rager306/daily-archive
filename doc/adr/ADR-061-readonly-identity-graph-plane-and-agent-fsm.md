# ADR-061: Read-only Identity Graph Plane and Agent FSM

## Status
**Accepted (M401-itcqhx/S01, 2026-08-18)** — implemented by
kg-storage `ReadOnlyServer` / `RemoteGraphStore` over the shared RESP2 codec
(`kg-storage::resp`), composed by `da serve-graph`, and pinned by the
cross-process contract test `crates/da-cli/tests/remote_read_plane_test.rs`.

Supersedes the vendor `samyama::RespServer` composition for the daemon path
(decisions D145, D146, D148) and refines the typed environment contract
(D149, D150). D127 (`import_eligible=false`), the no-RocksDB rule and the
no-auto-start rule are **not** weakened.

## Context

R095 (primary-user-loop, active): the agent needs a **read-only**
plan-read-synthesize-verify loop over the graph **without graph write
authority**. Before M401, the only multi-process surface was the vendor
`RespServer` (M308/M309) whose `handle_graph_ro_query` **delegated without
checking** into `handle_graph_query`: a raw TCP `GRAPH.QUERY CREATE` /
`GRAPH.DELETE` on `127.0.0.1:16379` mutated the daemon's shared
`Arc<RwLock<GraphStore>>` in full bypass of the client-side
`validate_readonly_cypher` and the `D127 locked` banner (threat-surface
finding Q3 in the slice plan). There was also no way for a second process to
verify it was talking to the *intended* graph: no identity handshake, no
semantic binding to a preload artifact.

Additional gaps found in the Q3 review:

- `--host` had no allowlist; `0.0.0.0` would open the LAN.
- `ServerConfig` had no `max_frame_bytes` / `operation_timeout`;
  `BytesMut` inbound buffering was unbounded (DoS via oversized frame).
- The client (`RemoteGraphConfig`) already bounded response size and
  timeout, but the server had no mirror limits.
- `GRAPH.LIST` discloses tenants and had to be deny-listed.

The slice must prove (runtime-executable): a second process reads the same
in-memory graph through a server-side read-only contract, verifies semantic
identity, and every mutation or admin command is rejected visibly without
changing graph counts.

## Decision

### 1. kg-storage owns the bounded RESP read plane

The RESP2 codec is extracted into `kg-storage::resp` (one parser/encoder
shared by client `remote.rs` and server `read_only_server.rs` — symmetric
wire contract, no drift). `ReadOnlyServer` is a generic fail-closed FSM over
`Arc<dyn GraphStore>` whose **only** accepted commands are:

```
PING  ECHO  INFO  DA.IDENTITY  GRAPH.RO_QUERY
```

`GRAPH.RO_QUERY` runs **server-side** `validate_readonly_cypher` before the
store is touched, so a hostile client that never runs the client-side
validator still cannot mutate. Every other command class — `GRAPH.QUERY`
(CREATE/DELETE), `GRAPH.DELETE`, `GRAPH.LIST`, `INDEX.*`, `VECTOR.*`,
`IMPORT`/`SNAPSHOT.*`, `ADMIN.*`, `CONFIG`, `SHUTDOWN`, `FLUSHALL`, `AUTH`,
`KEYS`, unknown commands, malformed frames, oversized frames, trailing-byte
frames — is answered with a typed `-ERR` **before** any store interaction.
Rejected writes therefore cannot change graph counts (proven by the
contract test).

Bounds (both server and client mirror):

| Bound | Server default (`DA_SERVE_GRAPH_*`) | Client default (`DA_REMOTE_GRAPH_*`) |
|-------|-------------------------------------|--------------------------------------|
| Endpoint / bind | `127.0.0.1:16379` (`HOST`, `PORT`) | `127.0.0.1:16379` (`ENDPOINT`) |
| Inbound frame cap | 1 MiB (`MAX_FRAME_BYTES`) | — |
| Outbound response cap | 8 MiB (`MAX_RESPONSE_BYTES`) | 8 MiB (`MAX_RESPONSE_BYTES`) |
| Operation timeout | 10 s (`OPERATION_TIMEOUT_SECONDS`) | 5 s (`TIMEOUT_SECONDS`) |
| Concurrent connections | 100 (`MAX_CONNECTIONS`) | — |

Unset keys fall back to defaults; blank / non-numeric / zero / overflow
values are rejected with a typed `EnvParseError` (never a silent default).
`da serve-graph` reads the server keys as env-aware clap arguments
(explicit CLI flag > env > default). Port `0` is refused by design.

### 2. Semantic identity handshake (DA.IDENTITY)

`DA.IDENTITY` returns a deterministic `SnapshotIdentity` JSON computed from
the **exported graph bytes** (`fingerprint_snapshot`): physical + semantic
SHA-256, semantic algorithm id, snapshot format version, tenant, node and
edge counts. The daemon computes it once at startup after optional `.sgsnap`
preload and fixes it for the process lifetime. A second process verifies via
`RemoteGraphStore::identity()` / `verify_identity(expected)`; a wrong
expected identity yields a **typed** `GraphStoreError::IdentityMismatch`
carrying **both** sides (expected + actual semantic SHA, tenant, counts) —
visible divergence, never an empty result. The semantic fingerprint is
stable across non-deterministic physical exports (proved by the preload
test), so the identity binds to graph *content*, not to a hand-assigned tag.

### 3. daemon composition (da serve-graph)

`da serve-graph` composes `ReadOnlyServer` over an in-memory
`SamyamaGraphStore` (through the storage port only — `Arc<dyn GraphStore>`).
Enforced by composition and type:

- **No RocksDB / PersistenceManager:** `ReadOnlyServerConfig` has no
  `data_path` field (compile-time), plus a runtime `refuse_persistence_path`
  gate on `DA_GRAPH_DATA_PATH` before bind.
- **No DirectGraphStore exposure:** the HOT mutable client never crosses the
  server path.
- **No vendor RespServer:** kg-storage owns the RESP plane.
- **No auto-start** from other commands and **no D127 unlock**
  (`import_eligible=false` everywhere; banners stay honest).
- Optional `--snapshot` preloads a durable `.sgsnap` into process memory
  (still dies with the process — NON-PERSISTENT banner).

### 4. Trust model

- Default bind is loopback; a hostile `0.0.0.0` never ships as a default.
- Protocol has no auth/nonce — requests are replayable; accepted for
  loopback by design (semantic identity is a verification aid, not
  anti-replay).
- Data exposure is low: Paper/Entity nodes, no secrets (FD_API_KEY etc. are
  redacted in health reports and never stored in the graph).
- The daemon exposes **no** DirectGraphStore client, **no** RocksDB path,
  **no** auto-start, **no** D127 unlock (must-have, verified by banners and
  the architecture guardrail).

### 5. Agent FSM readiness (R095)

S01 delivers the first half of R095: an honest read-only plane with semantic
identity, so a future agent can plan-read-synthesize-verify over the graph
without write authority. The agent command FSM (S02) and use-case wiring
(S03) build on this plane in later slices; this ADR pins only the plane
contract.

## Alternatives rejected

| Alternative | Why rejected |
|-------------|--------------|
| Keep vendor `RespServer` (D145/D146/D148) with client-side validation | Server-side delegation bypass proved by Q3 finding — mutation over TCP was possible in full bypass of `validate_readonly_cypher` and D127 banner |
| Patch vendor server to add validation | Vendor code lives outside the workspace; no bounded frames/timeouts; not reusable for the remote `GraphStore` client; kg-storage owns the universal contract (ADR-050) |
| Identity as a hand-assigned tag (e.g. `--name`) | Tag is not bound to content; a wrong preload could go unnoticed. Content-derived fingerprint closes the loop |
| Allow `GRAPH.QUERY` and rely on Cypher validation only | Validation alone is not a trust boundary; deny-by-default FSM is fail-closed |
| Auth/nonce handshake | Out of scope for loopback read plane; replayability accepted by design (documented) |

## Consequences

- **Positive:** one shared RESP2 codec; server-side fail-closed FSM closes
  the Q3 exploit; typed env contract prevents port drift (D149) and
  RocksDB creep; identity handshake makes "am I talking to the right graph?"
  answerable from a second process; `da health` under `DA_GRAPH_MODE=remote`
  stays honest (D150).
- **Negative:** protocol remains unauthenticated (loopback assumption must
  be documented for operators); snapshot preload is still memory-only
  (persistence is a separate concern, D127).
- **Supersedes:** D145/D146/D148 (vendor RespServer no longer used for the
  daemon path); **refines:** D149/D150 (typed `DA_SERVE_GRAPH_*` /
  `DA_REMOTE_GRAPH_*` / `DA_GRAPH_ENDPOINT` env contract).
- **Does not change:** D127 (`import_eligible=false`), no-RocksDB, no
  auto-start, no DirectGraphStore exposure.

## Acceptance evidence (M401-itcqhx/S01)

- `cargo test -p kg-storage --lib read_only` — incremental RESP parsing,
  read-only acceptance, every mutation class rejection, malformed/oversized/
  trailing-byte frames, multi-frame connections, bounded timeouts, mock state
  unchanged (T01).
- `cargo test -p kg-storage --lib identity` + `remote` — `DA.IDENTITY` JSON,
  `RemoteGraphStore::identity()`, wrong expected → typed `IdentityMismatch`,
  serde roundtrip, `from_env` rejects blank/non-numeric/zero/overflow (T02).
- `cargo test -p da-cli --bin da serve_graph` — banners no-RocksDB /
  no-auto-start / D127, env > clap precedence, preload identity, no
  DirectGraphStore on the server path (T03).
- `cargo test -p da-cli --test remote_read_plane_test` — **real
  cross-process**: process A = `da serve-graph` with a generated `.sgsnap`
  preload on an ephemeral port; process B = `RemoteGraphStore` for PING
  health, `DA.IDENTITY` semantic match with the preload artifact, and a
  scalar read query (`MATCH (p:Paper) RETURN count(p) AS papers`); raw TCP
  `GRAPH.QUERY CREATE` / `GRAPH.DELETE` and admin/index/vector/import
  commands rejected with typed `-ERR`; read counts unchanged; wrong expected
  identity returns a typed `IdentityMismatch` naming both sides (T04).
- `bash scripts/verify_rust_architecture.sh` — fmt + clippy +
  hexagonal invariants + per-crate tests green.
