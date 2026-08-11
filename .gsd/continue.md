# Handoff — 2026-08-11 (M381 semantic snapshot identity delivered)

## Delivered milestone

**M381-fociaz Semantic Snapshot Identity**

### Strict identity utility — `0963d5e4`

New `kg_storage::fingerprint_snapshot`:
- physical SHA256 of exact compressed `.sgsnap` bytes;
- strict semantic SHA `sgsnap-vid-canonical-json.v1`;
- bounded gzip decompression/record count;
- validates sgsnap v1/v2 header and counts;
- requires unique string VID on every node;
- resolves edge src/tgt physical IDs through VIDs;
- hashes labels, every node/edge property and topology;
- ignores only gzip/header time, record/map order and physical IDs;
- missing/duplicate VID, dangling endpoint, malformed/type/version/count/limit failures are explicit.

Real accepted snapshot:
- nodes=33, edges=33;
- repeated unchanged exports physical SHA differ;
- strict semantic SHA stable across exports/reload;
- one property mutation invalidates strict semantic SHA.

### LoadReport dual identity — `0963d5e4`

Universal da load now populates:
- legacy `snapshot_sha256` = physical compressed bytes;
- `snapshot_semantic_sha256`;
- `snapshot_semantic_algorithm`;
- snapshot node/edge counts;
- `snapshot_identity_ok` finding.

Identity is computed before report digest reseal. Invalid/VID-incomplete export makes report fail visibly. Previously `snapshot_sha256` existed but was never populated in universal load.

### Explicit replay Clock — `863f17df`

D227: strict semantic identity includes ADR-046 `created_at`/`valid_at`; never normalize temporal truth.

Real independent loads initially differed only by:
- 23 edge `created_at`;
- 9 fact `valid_at`;
- VIDs/topology/node properties identical.

D228 fix:
- `da load --materialization-time <RFC3339>`;
- parsed once at CLI boundary;
- one `Clock` injected into both ingest and extraction;
- default remains honest SystemClock;
- report records `materialization_time_explicit`.

Three-run exact-v1 proof:
- same explicit time, two fresh processes: strict semantic SHA equal;
- physical SHAs differ;
- one-second different explicit time: strict semantic SHA differs;
- D127 false.

## Verification

- snapshot identity: 3 unit/negative/permutation tests;
- real identity probe PASS;
- da-domain load report: PASS;
- da-cli snapshot identity: 2;
- da-cli load: 10;
- explicit clock unit/invalid test PASS;
- all-target clippy: zero project errors;
- full architecture guardrail: PASS;
- governor: zero violations; logs clean.

UAT:
- S01 `ef9a06f8-41a7-4624-9ff0-83e43a5325bd`
- S02 `00648101-9e7c-4d1e-b1ac-5e8050d4c768`
- S03 `d0f93cf6-9f04-4e80-bcd9-78e8bc43f1cb`

GitNexus detect rated load clock change HIGH (6 affected run_content processes); expanded load/full gate passed. Reindexed. Codebase Memory MCP timed out.

## Governor assessment

No regex. Strict semantic identity tests plus real temporal-difference diagnostics and injected Clock prove the product contract. A useful future governor enhancement would require a negative fixture showing a replay/equality report compares only physical SHA; then add a DATA rule requiring explicit identity class.

## Next product gaps

1. Audit remaining reports that call raw `snapshot_sha256` semantic replay evidence; migrate high-value assurance/checkpoint consumers to typed identity class.
2. Add a weaker structural digest only if explicitly needed; never substitute it for strict semantic equality.
3. Isolate dirty vendor Samyama tree before canonical byte serialization/server protocol work.
4. Consider source-policy default materialization time only as explicit ontology/config DATA, not hidden derivation.

## Locks

D127 false; D227 strict includes temporal truth; D228 explicit replay Clock; Samyama sole store; ADR-057 Proposed; ADR-058 Accepted; DSPy disabled.
