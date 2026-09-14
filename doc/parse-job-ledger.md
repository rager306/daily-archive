# Local parse-job ledger contract

## Scope

The local parse-job ledger is an operational control plane for parse execution. It
is not scientific graph truth and does not change `import_eligible=false` or any
D127 promotion route.

`JobSpec` separates immutable identity from operational details:

- identity binds the ordered content list, backend, projector, and profile;
- `key` is the caller-visible idempotency key and is normally derived by
  `JobSpec::identity_key()`;
- tuning and locations are deliberately excluded from identity;
- locations are operational hints and must not become authoritative paths or
  persisted payloads.

A repeated key with the same immutable identity is idempotent. A repeated key
with a different immutable identity is `Conflict`.

## Lifecycle proof contract

`JobRecord` carries status, monotonic revision, durable cursor, optional result
digest, and sanitized last error. An active `AttemptFence` contains revision,
monotonic epoch, opaque owner, and expiry. Every heartbeat, checkpoint, failure,
and terminal success must present the complete current fence. The adapter must
check the fence and apply the transition atomically before acknowledging it.

Terminal states are immutable. Durable `CancelRequested` competes with terminal
completion as one serialized transition; exactly one wins. A stale worker must
receive `StaleFence` and must not publish an authoritative cursor/result. Checked
counters and an authority clock must reject overflow, backward time, and expired
heartbeats rather than guessing.

The shared contract fixture exercises duplicate submit, key/spec conflict,
competing claims, stale heartbeat, cancel-before-terminal-write, terminal replay,
and invalid input. The fixture is a state fake only; it is not persistence proof.
The real adapter proof is `crates/da-adapters/tests/parse_job_ledger_test.rs`:
`reclaim_fences_stale_worker_and_terminal_state_survives_reopen` performs two
fenced claims without sleeping, rejects the old worker's checkpoint, rejects a
backwards cursor, commits terminal success, verifies terminal cancel replay, and
reopens the RocksDB-backed ledger to compare the recovered record and events.
This is a deterministic durable-CAS/restart proof; it does not claim to emulate
OS SIGKILL or an actual expired wall-clock lease.

## Feasibility boundary for the durable adapter

Source inspection confirms the current Samyama integration is process-local:
`kg_storage::SamyamaGraphStore::new()` wraps `SamyamaStore::new()` in an in-memory
`Arc<RwLock<_>>`. `open_for_mode(Embedded)` remains process-local, while non-empty
`DA_GRAPH_DATA_PATH` is rejected and remote writes are unsupported. The existing
`da-adapters` `redb` parse cursor is explicitly a rebuildable cursor/manifest
accelerator, not canonical state. Therefore neither Samyama HOT memory nor the
redb cursor can be claimed as the ledger authority.

T02 uses the existing `samyama::persistence::PersistenceManager` as that narrow
primitive. `PersistenceManager::new(base_path)` creates RocksDB-backed storage
and WAL directories; the RocksDB directory lock provides single-owner process
semantics, while `persist_create_node`,
`persist_update_node_properties_versioned`, and `checkpoint()` form the durable
ACK boundary. The adapter stores one serialized job record and an ordered event
list per `ParseJobLedger` node in the dedicated `daily_archive_parse_jobs`
tenant. A failed commit poisons the in-process ledger (fail closed) until the
caller reloads it; a second opener is mapped to typed `OwnerBusy` when Samyama
reports a lock failure. This is operational state only: no import, ingestion,
promotion, JSONL, redb, SQLite, or scientific graph writes are involved.

## CLI lifecycle (T04)

The opt-in `da parse-job` surface is deliberately separate from unmanaged
`da parse-batch`:

- `da parse-job submit --manifest <json> [--ledger <dir>]` reads the ordered
  manifest sources, derives a content digest for each logical document, and
  durably submits one `JobSpec`. Repeating the same immutable identity is
  idempotent; changing backend, projector, profile, or ordered content yields
  the ledger's typed conflict. `--tuning` and source locations remain
  operational and are excluded from the identity.
- `da parse-job status --job-id <id>` prints only persisted job/attempt,
  revision/epoch, cursor, result digest, sanitized error, and ordered events.
- `da parse-job cancel --job-id <id>` commits `CancelRequested` before any
  local worker wake-up. It is valid for queued and active jobs and is not a
  false success when another process owns the RocksDB lock (`OwnerBusy`).
- `da parse-job run --job-id <id> [--owner <opaque-id>]` claims or reclaims an
  expired attempt and prints the durable fence. It does not claim terminal
  success or execute an unmanaged batch behind the caller's back; the
  supervised application runner remains the authority for managed execution.

The ledger path is operational only and is never the scientific graph,
`import_eligible`, a queue file, JSONL, SQLite, or a redb authority.

## Runtime evidence and H06 boundary

H06 is source-bound to the durable Samyama adapter, not the older
`pipeline_ledger` planning surface. The current verification command is:

```text
cargo test -p da-adapters --test parse_job_ledger_test
```

The test covers durable ACK/reopen, idempotency and immutable-spec conflict,
competing claims, stale fences, monotonic cursor protection, cancel-before-
completion, bounded sanitized diagnostics, terminal replay, and event recovery.
The test does not claim application-level subprocess kill/reopen or a real
signal-driven CLI run; those remain separate evidence obligations for the
application/CLI surfaces. No scientific graph write or import route is involved.

## Observability and failure visibility

The port exposes typed `OwnerBusy`, `StaleFence`, `Cancelled`, `Terminal`,
`ClockBackwards`, `CounterOverflow`, and `Storage` failures, plus ordered
`JobEvent` records. Implementations should emit these decision outcomes without
including secrets, payloads, or absolute paths; storage failures must poison or
fail closed until reload rather than pretending an ACK was durable.
