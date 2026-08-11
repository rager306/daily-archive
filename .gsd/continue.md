# Handoff — 2026-08-11 (M384 closed; resume M383)

## Closed milestone

**M384-2kfqha Observable Scalable Graph Materialization**

Commits:
- `2cf50b3c` — progress heartbeat/stall visibility;
- `c8be7ef5` — temporal DATA cache + Samyama adjacency optimization + typed profiler;
- `34fca90e` — durable scaling profile;
- `1ff9ecf4` — SMP/storage/bulk feasibility docs.

### Operator visibility

`gold_graph_all_waves` now emits default 5s heartbeats and 30s stalls:
- phase/wave/current paper;
- completed papers and 3 work units per paper;
- elapsed/idle/throughput/ETA;
- mutation attempted/applied/unchanged/rejected;
- structured tracing plus stderr;
- final report schema v2 embeds progress.

10-paper progress on/off strict semantic SHA is identical. ABAB overhead was -0.48% (noise).

### Root causes and fixes

1. `edge_type_is_temporal_fact` loaded/parses ontology YAML for every edge.
   - before: 8,806 us/call;
   - after OnceLock DATA cache: 7.2 us/call;
   - 1,226x; all registry edge types parity-tested.
2. `SamyamaGraphStore::get_edges_between` scanned all graph edges.
   - replaced with vendor HOT+FROZEN adjacency `edges_between` reuse;
   - 3,000-edge missing lookup 512 -> 1.65 us;
   - 311x; contract/HOT/FROZEN tests pass.

### Real scaling

Same debug binary, warm cache, explicit time:
- 1 paper: 0.11s;
- 10 papers: 0.63s;
- 30 papers: 1.56s;
- 200 papers: **18.59s** (prior M382 ~57 minutes; ~184x).

200-paper acceptance:
- CPU 113%; RSS 229 MB;
- graph 23,836 / 39,295;
- 504,965 observed mutation attempts (~27,163/s);
- strict semantic SHA equals durable accepted graph;
- replay applied=0;
- heartbeat complete, no stalls;
- D127 false.

Durable evidence: `artifacts/performance/materialization_scaling_m384.json`.

### SMP

30-paper worker matrix, strict identity/graph/mutations all identical:
- workers 1/2/4/8/12;
- wall 1.74/1.53/1.40/1.58/1.48s;
- CPU 101/110/118/119/121%; RSS 43-49 MB.

Keep `prepare_workers=4`. Preparation is parallel; graph mutation remains one writer. More workers do not help this warm workload.

### Bulk and RocksDB verdicts

- Reuse existing `OrderedMutationPlan`; do not create another DSL.
- Samyama MVCC abort does NOT roll back in-place ordinary writes; a wrapper would be falsely atomic. Bulk deferred until clean vendor atomic batch or COW store swap with rollback contracts (D235).
- No current daily-archive product runtime opens RocksDB:
  - `serve-graph` data_path=None;
  - DA_GRAPH_DATA_PATH -> PersistenceNotConfigured;
  - remote HOT unsupported.
- Therefore RocksDB tuning is not applicable now (D234/D236). Future persistent daemon milestone must first establish vendor isolation, crash recovery and real workload.

Verification:
- full architecture guardrail PASS;
- governor zero violations (only resource/STATE drift warnings);
- application errors 0;
- GSD failed exec 0;
- GitNexus reindexed after additive profile/progress symbols;
- Codebase Memory MCP timed out repeatedly; supplemental unavailable.

## Active milestone

**M383-uber46 Revision Owned Corpus Acceptance** remains active.

### Exact next action

Execute M383/S01 Revision Migration Contract:
1. inventory all 200 gold records against canonical catalog provenance;
2. classify explicit revision and source evidence;
3. never assume v1 for versionless assets;
4. versionless policy belongs in ontology DATA;
5. emit bounded per-paper decisions/digest and fail closed on ambiguity.

Then use the now-fast observable materializer for S02 revision-owned rebuild, S03 grounded revision summaries, S04 five-class ontology acceptance.

## Locks

D127 false. D227 temporal properties are strict identity. D228 explicit replay Clock. D229 exact physical lineage. D230 Citation namespaces. D231 M384 before expensive M383 rebuild. D232 cached immutable ontology DATA. D233 algorithmic fixes before bulk. D234/D236 no inactive RocksDB tuning. D235 no pseudo-atomic bulk. Samyama sole store; no legacy Paper summaries; no implicit revision hardcode; DSPy disabled.
