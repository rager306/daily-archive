# Handoff — 2026-08-11 (M378 remote scalar read plane delivered)

## Delivered milestone

**M378-pfsidy Remote Samyama Read Plane**

### S01 `722ca5f1`
- New universal `kg_storage::RemoteGraphStore` over bounded RESP.
- Real `health`, `query`, `query_readonly`; all vector/index/snapshot capabilities explicit NotImplemented.
- Incremental parser: partial/malformed/error/wrong-width/UTF-8/null/integer tests.
- 5s default timeout, 8 MiB response cap, no retry.
- Fail-closed Cypher classifier rejects CREATE/MERGE/DELETE/SET/REMOVE/DROP/CALL/LOAD/FOREACH/USE before connect.
- Vendor GRAPH.RO_QUERY delegates to GRAPH.QUERY, so local guard is mandatory.
- D226: integers/null typed; bulk strings stay strings. Bool/float cannot be inferred safely from current vendor wire.
- Live PING/revision/count proof against snapshot-preloaded serve-graph.

### S02 `486a3e34`
- Added `da graph-query --mode embedded|remote` separate from CRITICAL HOT `open_graph_store`.
- Explicit mode or DA_GRAPH_MODE; remote requires endpoint/DA_GRAPH_ENDPOINT and never auto-starts daemon.
- Embedded supports same-process snapshot; remote rejects snapshot.
- JSON stdout clean. Configuration exits 2; runtime/protocol/query-policy exits 3.
- Live unambiguous integer query remote == embedded: revisions [1,5].
- Health/serve banners updated: scalar read available, full/HOT remote still NotConfigured.

### S03 `0ec2a62b`
- Probe reports capability matrix, before/after counts, 8 concurrent reads and duration.
- Two repeats: 16 concurrent reads, every count=2; before=after=2; revisions stable.
- CREATE rejected locally; index/snapshot explicit unsupported.
- Full architecture guardrail PASS; governor zero violations; logs clean.

## Verification and UAT

- kg-storage remote tests: 10.
- da-cli graph-query tests: 2; health/serve 10 each; da-cli lib 73.
- all-target clippy kg-storage+da-cli: 0 project errors.
- `scripts/verify_rust_architecture.sh`: PASS.
- UAT S01 `74b97eb7-0da1-4cdc-8d90-bbefce026e6b`
- UAT S02 `cbb924f1-7612-4c99-a31e-57075d095f92`
- UAT S03 `6d829f17-65ef-4efd-8ec0-0de656582381`
- GitNexus reindexed after structural waves; final S03 detect LOW.
- Codebase Memory MCP timed out; supplemental unavailable.
- serve-graph launched for proof and stopped cleanly.

## Governor assessment

No new governor regex. The key risk (vendor RO_QUERY not read-only) now has executable negative tests proving local rejection before connect. A governor rule would be weaker than this product contract unless a future alternate remote adapter bypasses `validate_readonly_cypher`; add a DATA-backed architecture check only after such a negative fixture exists.

## Next product gap

Remote plane is intentionally scalar read-only, not full GraphStore. Next wave should be chosen by need:
1. Prefer server protocol improvement: typed RESP envelope (bool/float/node/edge) plus server-enforced RO_QUERY.
2. Then add remote vector/index/snapshot only with real server commands and shared contract tests.
3. Do not wire remote into HOT DirectGraphStore until atomic mutations, temporal stamping and transaction semantics exist.

## Locks

D127 false; Samyama sole store; no hidden fallback/auto-start; remote writes unsupported; ADR-057 Proposed; ADR-058 Accepted; DSPy disabled.
