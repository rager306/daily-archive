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

## Feasibility boundary for the durable adapter

Source inspection confirms the current Samyama integration is process-local:
`kg_storage::SamyamaGraphStore::new()` wraps `SamyamaStore::new()` in an in-memory
`Arc<RwLock<_>>`. `open_for_mode(Embedded)` remains process-local, while non-empty
`DA_GRAPH_DATA_PATH` is rejected and remote writes are unsupported. The existing
`da-adapters` `redb` parse cursor is explicitly a rebuildable cursor/manifest
accelerator, not canonical state. Therefore neither Samyama HOT memory nor the
redb cursor can be claimed as the ledger authority.

T02 must first introduce or identify a narrow approved Samyama durable commit and
single-owner process-lock primitive. It must not silently promote redb, JSONL,
SQLite, a fake remote store, or the scientific graph plane into authority. Until
that primitive exists, this T01 port remains the authority-neutral contract and
its fake tests only prove transition semantics.

## Observability and failure visibility

The port exposes typed `OwnerBusy`, `StaleFence`, `Cancelled`, `Terminal`,
`ClockBackwards`, `CounterOverflow`, and `Storage` failures, plus ordered
`JobEvent` records. Implementations should emit these decision outcomes without
including secrets, payloads, or absolute paths; storage failures must poison or
fail closed until reload rather than pretending an ACK was durable.
