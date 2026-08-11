# Handoff — 2026-08-11 (M382 closed; M383 planned)

## Closed milestone

**M382-8j6eiq Checkpoint Snapshot Lineage**

Commits:
- `34f56f3f` — typed checkpoint identity + production identity evidence;
- `126d5183` — exact DATA-governed lineage, Citation VID repair, honest 200-paper rebuild.

### Product changes

- Corpus checkpoint wire schema is now v3.
- Snapshot evidence carries exact physical SHA, strict semantic SHA, algorithm, path and counts.
- `da snapshot-identity --input ... --report ...` emits bounded atomic self-digested evidence.
- Derived snapshots require `data/ontology/snapshot_derivation_policy.yaml` and exact physical producer continuity.
- Removed `--allow-derived-snapshot`; reordered/final-mismatch chains fail before import/tasks/package replacement.
- Governor rule `no_boolean_derived_snapshot_waiver` prevents the bypass returning; negative fixture included.

### Real ontology defect repaired

M381 strict fingerprint exposed 173 global VID collisions in the accepted 200-paper graph:
- every collision was a Paper/Citation pair sharing `paper_vid`;
- added `citation_arxiv_vid` namespace (D230);
- malformed arXiv claims remain observable Reference-only and do not create Citation nodes/edges;
- 188 malformed claims observed during rebuild;
- durable base/cluster/resolution artifacts rebuilt through real producers, not rewritten.

Current durable graph:
- base: 23,836 nodes / 39,295 edges;
- resolution: 24,527 nodes / 40,623 edges;
- duplicate VID: 0;
- direct checkpoint: PASS, 0 derivation links;
- derived checkpoint: PASS, 2 exact links;
- D127 false.

Verification:
- domain checkpoint tests 9;
- application checkpoint tests 4;
- batch ingest tests 17;
- strict durable identity PASS;
- full architecture guardrail PASS (`/tmp/m382-arch2.out`);
- governor violations 0 (resource/STATE warnings remain);
- application errors 0; recent GSD failed exec 0;
- UAT: `cd48aa9a-90b5-4407-8893-5ef61669a936`, `d5d4bc2e-cddd-4348-b83f-1165e919b3e8`, `60e1eb96-d335-4c2a-811c-a5f16c0772e4`.

Operational lesson: real 200-paper materialization rebuild took ~60 minutes even with cache. Do not run it before focused fixtures; do not run concurrent Cargo jobs (one accidental concurrent pair only waited on target lock).

Codebase Memory MCP timed out on all narrow calls this session; GitNexus authoritative. Reindex: 23,536 nodes / 39,084 edges / 550 clusters / 300 flows.

## Active milestone

**M383-uber46 Revision Owned Corpus Acceptance** planned with four slices:
1. S01 Revision Migration Contract
2. S02 Revision Aware Materialization
3. S03 Corpus Grounded Summaries
4. S04 Full Corpus Acceptance

### Exact next action

Plan and execute M383/S01:
- inventory all 200 gold materialization records against canonical catalog provenance;
- classify explicit version/source evidence;
- do NOT assume version 1 for versionless assets;
- policy for versionless handling belongs in ontology DATA;
- output bounded per-paper evidence/digest and fail closed on ambiguity.

Then focused fixture before touching central HOT materializer.

### Why M383 exists

After honest M382 rebuild, `ontology_acceptance` now fails only:
- required class `summary`: EMPTY/Failed;
- structural/fact/cluster/resolution remain available.

The old corpus summary producer (`extractive-summary-canary.v1`) was lost when summaries became revision-owned. Git history version no longer compiles because current `ExtractiveSummaryUseCase` requires `PaperRevision` and source VIDs. Do not restore Paper-owned summaries. M383 must migrate corpus content to revisions, create a current revision-owned corpus summary process, rebind ACAP exactly, then pass all five classes.

## Locks

D127 false; D227 strict identity includes temporal truth; D228 explicit replay Clock; D229 semantic equality never replaces physical provenance; D230 Citation namespaces are source-qualified. Samyama sole store. No legacy Paper summaries. No implicit revision hardcode. DSPy disabled.
