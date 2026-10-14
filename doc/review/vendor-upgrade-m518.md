# M518 vendor update — source audit and bounded compatibility checks

The source audit selected exact `origin/main` commits. Library checks passed for kg-storage with Samyama/algorithms and for CLI/MCP before the independent review corrections. The corrected sources passed 152 storage tests, 10 checkpoint tests and 60 PPR/embeddings tests. Native vector, snapshot and community units also passed: 49, 90 and 3 tests. Bounded String-ledger old→new compatibility also passed. Standalone critic passed 22 non-model tests; root adapters passed 239 tests and the four current-generation ledger contracts passed. Feature-enabled storage clippy passed after an equivalent predicate repair; the storage suite repeated with 152 passes and MCP passed 47. The current full native architecture gate passed23/23 with exit0 and no timeout. Typed final artifact UAT and four fresh hermetic ledger contracts passed. Supported source-bound GSD acceptance remains pending. Both shared vendor trees are preserved. Frozen M517 results remain unchanged. No live graph, operational store, GGUF model or production artifact was opened or rewritten.

| Vendor | Original commit | Audited candidate | Latest release recorded separately |
| --- | --- | --- | --- |
| Samyama | `fd45131b3e1d3e58e8a6ff1ee7618d872477e79d` | `17e55660ef90bc177ac9b55a1a22a9d39b41aa0f` | v1.11.0, `4c99eb364fc919366e60b268b611390518b63566` |
| RuVector | `c6bb23c842d7c220e06e6141d5e55ae52077fe12` | `bfb257ab9e08db006e9f4c656580bf1190a5d6f9` | WASM binding 2.3.1, `06c23951b15d33ef89d38dd1203fb48d5326c403` |

`data/vendor_sources.yaml` records pins, candidate paths, patch hash and verification status. This file is the project record, not a claim that external paths are immutable. Recreate candidates with detached `git worktree add` at the recorded commits, apply the recorded Samyama patch, and retain the native RocksDB configuration before compiling. Do not pull, reset or clean the shared originals.

## Evidence before the first build

Samyama has two required local vector commits and 249 dirty tracked files. Native rustfmt comparison of temporary copies classified 245 Rust files as formatting-only, one as re-export order/spacing, and one as a semantic WCC predecessor-union patch. The remaining changes are Cargo development profiles and agent documentation. Original binary patches and untracked inventory are retained in `/tmp/vendor-upgrade-audit`.

RuVector complete ancestry was established by authorized unshallow: no local-only commits, 286 upstream commits, current HEAD is the merge base. Earlier 1-ahead results came from a shallow boundary and are not local patches. Whole root-consumed sona, hybrid, solver and cognitive-container source trees are unchanged. The standalone critic changes `ruvllm` 2.3.0→2.5.0 and core 2.3.0→2.4.0. Reflection APIs are unchanged, but Candle/GGUF and ordinary HNSW FlatGraph behavior differ. No model inference or behavior equivalence is asserted.

Fresh GitNexus central identity checks found zero ID/path mismatches across 107,087 filepath nodes. Struct/trait dispatch remains UNKNOWN, not unused. Exact source confirms current consumers. Snapshot identity and node resolver impacts are CRITICAL; domain identity validation and vendor vector event insertion are HIGH. These warnings were delivered before edits. Vendor index skips giant operator/planner files and caps some receiver/process analysis, so graph results remain lower bounds.

## Required source adaptations

- Restore duplicate-vector refusal and explicit same-node replacement in the new F32/F16 backends. Rebuild storage, HNSW and exact buckets before swapping the index. Preserve manager names/model bindings. Missing-index replacement now errors rather than the previous local false-success gap.
- Keep F32 defaults. F16 equality compares the stored rounded half-precision bits; it is not f32 identity. No fp16, GPU or SIMD feature is activated. WCC retains both incoming/outgoing unions. Numeric union roots are not stable component identities.
- Read node and edge properties through upstream column-first getters. Old sparse edge-map reads would lose current weights and temporal strings.
- Recurse through query List/Map values. Newly introduced temporal values use explicit `__type` and all numeric/zone components, matching the pinned snapshot representation. Existing DateTime, vector, duration and materialized entity-placeholder JSON remains unchanged. This does not make old placeholders a lossless entity projection.
- Preserve `sgsnap-vid-canonical-json.v1` for snapshots without declaration records. Add explicit v2 when any index catalog, even empty, or hierarchy record exists. Absent and empty catalogs differ during restoration. v2 binds typed declarations and catalog presence; unknown declaration types/fields/versions still fail. Domain accepts exact v1/v2 only; evidence digests and D127 checks remain required.

The reviewed candidate patch is `data/vendor_patches/samyama-m518-vector-wcc.patch`, SHA256 `23deec4f7d25c16379f4f0e4a5fc73534e22a8a8a538a3f6751561b6db3c3a9e`. The filename is retained; the patch now also contains the column-first snapshot export repair and its regression test. The previous vector/WCC-only patch is preserved in the audit directory. Native format checks passed on source edits. No compiler/test result is implied by formatting.

## Rollback boundaries

A source rollback must revert the complete M518 source/manifests/locks change set to the pre-update commit `eda16b4a496d8c177d2d6b1ab57e5feccf52a4cd`, then rebuild. Restoring only the vendor paths or lockfiles is insufficient: adapter enum matches and metadata handling now use the new API. Before a commit exists, inspect and preserve unrelated work rather than using a broad reset. After a single M518 commit exists, revert that commit as a reviewed source change; do not pull/reset either shared vendor original.

Original vendor checkouts and the pre-update root/critic locks remain preserved. Candidate recreation is the exact recorded commit plus the tracked patch, not a moving `main` checkout. Full five-file apply reconstruction now produces candidate bytes exactly; this is source reproducibility, not a replacement for runtime checks.

No operational graph or production ledger was migrated, so no production data rollback is needed for this work. A new engine opens only disposable ledger copies. Never point the old engine at a new-engine writable store, and do not infer downgrade safety from the String-only compatibility probe. Keep the old original closed; recovery starts from an independently preserved old backup. Generic binary migration and downgrade remain forbidden.

## Persistence boundaries

RocksDB crate 0.22 / librocksdb-sys 0.16+8.10 and system library 8.10.0 stay unchanged. Samyama inserted five PropertyValue variants before old array/map/vector/duration/null binary ordinals. Old unversioned PropertyMap bincode is not generally compatible; wrong-value decoding can succeed silently. General legacy RocksDB in-place update/downgrade is forbidden here. No generic migration is implemented.

The scientific KG adapter is process-local. `SamyamaJobLedger` is a real separate RocksDB consumer. Its state/spec/events are JSON inside unchanged String ordinal 0; copied old→new exact spec/record/events and mutation/reopen checks passed for four disposable records. Default unsynced durability, new WAL envelopes and old-reader allocation hazards remain outside a lossless downgrade claim. New writable stores must never be handed back to the old engine in place.

Plain `.sgsnap` header version remains 2. Artifact tags kg-snapshots-v9/v10 and software 1.10/1.11 are not format generations. Snapshot import appends/remaps; restore only into a fresh disposable store. New temporal/index/hierarchy semantics are lossy in older readers. Snapshots still omit history/edge timestamps, trim strings and do not establish durable import. The project bounded RESP read plane is retained; vendor RESP is not substituted.

## Dependency and resource checks

Reviewed lock resolution uses Samyama 1.11.0, Arrow/Parquet 53.4.1, chrono 0.4.39, registry HNSW 0.3.5 and lru 0.18.4. Arrow requires chrono <0.4.40. Upstream requires crc32fast 1.5.2, so retaining 1.5.0 was refused by Cargo. Unrelated libredox 0.1.25 was reverted to 0.1.19 through a targeted Cargo update. Parquet adds zstd 0.13.3 and resolves zstd-sys 2.0.13+zstd.1.5.6; the system RocksDB remains 8.10.0. Root-used RuVector versions are unchanged. The critic lock changes only path-package versions: ruvllm 2.5.0 and ruvector-core 2.4.0. All its registry entries remain identical.

The vendor HNSW patch is not inherited by consumers. Native symbol unification/linking must be checked; it is not guaranteed by Git pins. Rust toolchain is 1.99; declared workspace 1.85 support remains unproved. Mimalloc becomes a default dependency, but the embedded library does not install a global allocator.

User requires sequential work. Every operation records free space before/after against a 15 GiB floor. No parallel compiler/index/implementation job or sibling cache cleanup is permitted. The sibling allspark owner completed and cleaned its own build before M518 resumed.

## Independent source review and corrections

The source-only review found two correctness blockers and two test compile defects. Snapshot export was row-first while the authoritative getter was column-first. The isolated candidate exporter now reads the existing merged property view; the conflicting upstream test now asserts the live and restored column value. Wrapper impact is CRITICAL (218 symbols, 85 direct callers, 13 processes). Exact implementation impact is CRITICAL (213 symbols, two direct callers, 12 processes); expanded queries are not truncated. Warnings preceded edits.

Vector names that collide after the pinned manager's trim rule now refuse, in either catalog order. Hashes still retain declared name text. Two integration tests now qualify the port's vector search and unwrap GraphView with its existing CSR count. A new real export/fingerprint/fresh-import fixture declares all four catalog types and a hierarchy, then checks declarations, named vectors and hierarchy rollups. This fixture and the other four compatibility tests passed in the feature-enabled storage suite. The normalized-name refusal, temporal conversions and legacy/v2 identity fixtures also passed. No loss-detection assertion was weakened.

## Verification status

| Completed check | Result | Boundary |
| --- | --- | --- |
| kg-storage, `samyama,algorithms`, `--tests` | 152 passed, 0 failed; 469.5 s | Includes shared Mock/Samyama contracts, 108 units and all five vendor compatibility tests |
| da-domain `corpus_checkpoint` | 10 passed, 0 failed; 127.2 s | Exact v1/v2 acceptance, digest binding, D127; 857 other tests filtered |
| kg-algorithms + kg-embeddings, `ppr`, `--lib` | 39 + 21 passed, 0 failed; 46.0 s | Consumer PPR/CSR/RuVector contracts, not generic vendor behavior |
| Standalone critic, model tests excluded | 22 passed, 0 failed; 2 filtered; 621.5 s | Reflection, subject and parsing contracts; no inference behavior proof |
| Root adapters default library | 239 passed, 0 failed; 1 ignored; 478.6 s | Includes SONA/RVF/WASM adapters; feature-gated suites are separate |
| Root ledger current-generation contracts | 4 passed, 0 failed; 0.92 s runtime | Fresh temporary stores; no SIGSTOP or lease changes |
| Candidate patch recreation | `git apply --check` exit 0 on exact upstream files | Candidate diff exactly equals tracked patch SHA256; no compiler proof implied |

There are878 scoped passing test executions:736 consumer and142 native vendor units. The final architecture gate separately recorded7,629 passing test executions across repeated feature planes; these are not new unique tests and are not combined with878. This includes a 152-test storage repeat after the equivalent lint repair, not 152 new unique tests. The four-record cross-generation ledger probe is separate from that count. Raw logs and source-bound receipts are in `/tmp/vendor-upgrade-audit`. The first library checks predate the exporter correction; the storage test run uses the corrected sources.

The first native vendor vector compilation was interrupted for disk recovery before any test result. Its stopped process tree was terminated after ownership checks. The restart passed all 49 tests selected by `vector::`, including local HTTP vector tests. Native snapshot units passed 90 tests, including the column-first exporter regression. Community units passed three tests, including edges present in only one CSR array. The native vendor lock has HNSW 0.3.4, whereas consumer tests use 0.3.5; these results must remain separate. Native runtime profile options match the root's debug level and disabled dependency debug/incremental compilation, but no vendor manifest or lock was changed.

Current application/CLI default, pdf-layout and learning-evaluation suites passed in the full23-step gate. MCP passed its separate47-test library run; feature-enabled storage clippy passed. Final current-source graph identity/parity passed, with documented coverage limits. Typed artifact/native ledger UAT passed; source-bound GSD acceptance is pending. Lock resolution alone is not runtime proof, and inherited governor drift is not an upgrade success.

The prepared ledger probes compile the exact current adapter against each vendor generation. They compared spec, records and events for running, succeeded, cancel-requested and failed jobs on copied, closed temporary stores. New-generation mutations and reopen ran only on a separate new-engine copy. The old original remained closed and hash-checked. This is String-only JSON compatibility, not general PropertyValue migration or crash-durability proof. The old probe compiled successfully. `ldd` resolves `librocksdb.so.8` to the system `/usr/lib/x86_64-linux-gnu/librocksdb.so.8.10.0`. The old probe generated four records in a fresh disposable store, closed it, then verified a separate old-engine copy: exact specs, records, events, idempotency, conflict refusal and exclusive-owner lock. The three planned copies matched the closed original before any open. The closed original and expected JSON hashes remain unchanged. The new probe compiled, linked the same system RocksDB 8.10.0 and passed exact verification on one copy. On another copy it passed same-owner reclaim, stale-fence refusal, completion and reopen. All four runtime commands exited 0 in under 0.5 seconds each; no active lease was paused or extended. Closed original and expected JSON hashes remained unchanged. No new writable store was given back to the old engine; no production store was opened.

Standalone critic source inspection found exactly two tests that load a model. Both were excluded explicitly, regardless of local model presence. All 22 non-model tests passed; two were filtered. Root adapters passed 239 tests, with one ignored. The four current-generation ledger integration tests passed after a separate compile-only step; their 0.92-second runtime was not suspended and the 30-second lease was unchanged. `gguf-smoke` and `observe-gguf` remain forbidden; non-model parsing, subject, registry and reflection tests do not establish Candle/FlatGraph inference equivalence.

## Resource events

The first storage test build reduced observed free space from 25.28 GiB to 10.43 GiB, below the required 15 GiB floor. Before/after checks failed to prevent this. Further builds stopped, and supported compiler-cache cleanup restored capacity without removing target artifacts, measured runners, evidence or source data.

The in-flight guard now checks free space every two seconds. At a 20 GiB soft threshold it pauses only the current owned process tree; the required floor remains 15 GiB. This guard paused the vendor build at 19.91 GiB before the floor was reached. The stopped build was later terminated only to allow compiler-cache cleanup without live compilers. Exit -9 is a resource interruption, not a test failure or pass. Other agents resumed builds after earlier completion acknowledgements, so the cache cannot be cleaned safely until their compilers finish. Foreign processes and targets are never signalled or removed. User requested useful non-compiler work during this hold; patch recreation, source/lock consistency checks, ledger/critic preparation and verification records continue sequentially while the owned compiler remains stopped or has been ended for disk recovery. A shared-host acknowledgement is not a resource observation; native process and disk checks precede resume.

No deployment, import, auto-promotion, graph-data write, corpus recapture, timing replacement, scientific-quality gain, speedup or push is authorized by this dependency update. Audit receipts: `/tmp/vendor-upgrade-audit`; resume checkpoint: `/tmp/vendor-upgrade-audit/checkpoint.txt`.


The follow-up review found no new blocking source defect. It checked current source hashes and raw logs, but ran no compiler or test. Its ledger review confirms the bounded compatibility oracle, not an independent lifecycle oracle. The runner must enforce separate disposable writable copies and hash the closed original before and after. Active create/complete intervals use the unchanged 30-second wall-clock lease; no process suspension or lease extension is permitted during those short operations.

A later supported `kache clean --cache --yes` recovered only 0.02 GiB, so cache cleanup alone was insufficient. A native `cargo clean --dry-run` confined the planned removal to this repository's generated CLI/parse target files. `cargo clean --locked --offline --target-dir /root/daily-archive/target -p da-cli -p da-parse` then removed 30.8 GiB of generated artifacts. Observed free space rose to 45.6 GiB. The tested compatibility executable and shared Samyama dependencies remained; the two frozen M517 runner hashes remained identical. No foreign target, source, corpus, proof log or production store was removed.

The current process-local compatibility executable has no RocksDB `NEEDED` entry. Its successful runtime tests do not prove native persistence linking. Both native ledger executables separately resolve `librocksdb.so.8` to system 8.10.0, and the copied-store runtime checks passed. This does not change the process-local executable’s proof scope.

Project-owned formatting passed for all 12 kg-*/da-* packages and the standalone critic. `cargo fmt --all -- --check` failed only on 431 untouched upstream Samyama files, with no project-owned difference. The GSD verify field now uses the same explicit package scope as governor and pre-commit. Untouched vendor files were not reformatted; the failing broad check is retained.

The first feature-enabled storage clippy run failed on one `collapsible_if` in the new declaration helper. Pre-edit graph analysis reported CRITICAL for its enclosing fingerprint function; the unindexed private helper’s sole caller was confirmed in source. The nested condition became an equivalent `Option::is_some_and` predicate, avoiding a let-chain that would require Rust newer than 1.85. No runtime constant or test assertion changed. Clippy then passed and all 152 storage tests repeated successfully on source SHA256 `fbf14f96b81a0cfc21bc72df81e5598d9e4fcf2e762ab2fac796233156faaa37`. Earlier receipts remain tied to their original source bytes.

Before the full gate, supported compiler-cache cleanup recovered 10.6 GiB: observed reserve 23.79→34.38 GiB. No target or source cleanup was applied; frozen runners and the closed old-store hash stayed unchanged. The full gate uses invocation-only `CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0` to omit debuginfo. Optimization, debug assertions, test selection, source profiles, lease and disk thresholds remain unchanged. This profile change is a disk safeguard, not a speed or equivalence claim.

## Full architecture attempt — failed and resource-interrupted

The full gate completed 19 steps: 17 passed and two CLI suites failed on the same two `governed_process_record_test` cases. Both stop at `journey vector attach` because `Paper.embedding` has no declared vector index. The new vendor correctly refuses this absent-index write; weakening that error or the assertions is not an accepted repair. The caller’s index initialization is under investigation. This is a newly observed compatibility failure, not accepted inherited drift.

During step 20 compilation, the owned-tree guard paused at 19.83 GiB, above the 15 GiB floor. All four stopped owned processes were checked by parent tree, current command and state, then terminated for safe cache cleanup. Exit -9 is a resource interruption, not a result for step 20. Steps 21–23 did not run. No active ledger runtime was suspended and no foreign process was signalled. Supported cache cleanup with no live compiler recovered reserve to 25.75 GiB; targets, frozen runners and the closed old-store hash remained unchanged. The gate is failed/incomplete, not passed. Raw completed-step captures and the interruption receipt remain in `/tmp/vendor-upgrade-audit`.

## Consumer repair — absent journey vector index

A read-only graph/source review found no vector-index initializer on the production journey path. `ProductionContext` preserves a bare process-local store; `schema_init` opens a different store. Repairing only test fixtures would leave `da load --journey` broken. The preferred `run_journey_content` edit has MEDIUM impact (10 resolved upstream symbols, six direct callers, one process group). The alternative shared application `write_node_embedding` seam has CRITICAL impact and was not changed. These counts do not cover unresolved async/trait calls.

The first-attachment branch now checks for `Paper.embedding` and creates it only when absent, under one concrete-store write guard. It uses the injected embedder dimension and the existing CLI cosine policy. The guard is released before the existing add and successful-write-only SHA stamp. No generic adapter auto-indexing or product label was added to kg-storage. The vendor's missing-index refusal remains unchanged.

This repair assumes that a fresh journey uses the existing cosine policy. A predeclared index remains authoritative; dimension conflicts still fail. Existing vectors, names, quantization and model bindings are not reset. The repair neither introduces model-mismatch enforcement nor backfills historical SHA-stamped nodes whose old missing-index write stored no vector. No constants, graph-write/import locks, production stores or M517 artifacts changed.

The bare-store replay test now requires a positive nearest-neighbor hit and vector count before and after replay, not just a SHA property. Two new state-based cases check preservation of a populated named FP16 index/model binding and refusal of a conflicting dimension without replacing the index or stamping the new Paper. All existing assertions remain. The repaired sources were formatted and both focused targets passed: 11 test executions under default features and the same 11 under `pdf-layout`. The cases include both previously failing governed-process tests and the two new vector-state tests. The earlier foreign-compiler hold was followed by independent process clearance and supported own CLI cleanup; no production data or frozen evidence changed. This adds 22 successful scoped executions (850 total, including repeats, not unique tests). The focused `pdf-layout` clippy check also passed (exit 0). The first independent repair reviewer ended without a report or reply, so that run receives no review credit. The bounded replacement review completed and found no material defect in the narrow journey repair. The broader concrete-root review below and current native23 gate supersede these pre-helper sources. The earlier architecture attempt remains failed/incomplete; final GSD acceptance is not inferred.

## Broader content composition repair — bounded verification

The independent journey review found no material defect in that narrow repair, but source-supported legacy `Paper`/`PaperRevision` write paths also opened bare stores. Whole-runtime upgrade acceptance cannot rely only on the repaired journey. A concrete CLI `ensure_embedding_index` helper now owns the absence-only check/create and refuses an existing dimension mismatch before writing. It leaves existing vectors, metric, name, precision and model binding unchanged. Journey uses the helper; the universal adapter and CRITICAL shared application writer were not changed.

The helper is wired into single and batch ingest, legacy load and the scheduler content root. Online declarations use the configured embedder's dimensions. The two known offline materializer examples also declare their `PaperRevision` vectors: the revision canary reuses its supplied two-dimensional vectors, while the catalog runner uses the existing nonempty cached vector dimensions and routes refusal through its per-paper failure FSM. No new model enforcement or historical backfill is claimed. Optional Entity vector readiness is still a separate limitation; no live store or corpus replay was opened.

Three new real-embedded composition tests cover strict missing-declaration refusal, fresh Paper/PaperRevision materialization and vector replay, and preservation of a populated named FP16 index through dimension refusal. The earlier 22 journey passes and clippy result apply to the pre-helper revision. The current helper/wiring sources were formatted, and all 14 focused tests passed under both default and `pdf-layout` features (28 successful executions). These include three new real-embedded composition cases and the two existing governed-process tests. The binary, three focused test targets and both offline materializer examples passed clippy after one equivalent batch-loop correction. The initial exit-101 `collapsible_if` result is retained; the correction skips unneeded labels with `continue` instead of nesting the failure branch. It changes no constants or assertions and does not add a newer-Rust let-chain. Only main.rs changed after the 28 test executions; helper, journey and test sources remain byte-identical. The independent current-source wiring review found no material blocker for the named content roots. All seven source hashes match current clippy; the reviewer reversed the main-only control-flow rewrite in memory and reproduced its earlier runtime-test hash exactly. That review proves source equivalence, not post-flatten runtime. The later complete native23 gate now executes current-source CLI default/pdf-layout/learning-evaluation tests and passes. Runtime execution of the two materializer examples remains unclaimed. No constants or import/deployment/graph-write locks changed.

## Final-gate resource preparation after the content repair

Supported own-package cleanup and compiler-cache cleanup recovered reserve to 33.53 GiB. Every dry-run path was under `/root/daily-archive/target` and protected runner/contract/probe paths were excluded. Frozen M517 runners/artifacts, both retained native probes, the ledger contract runner, vendor patch and closed old-store hashes remained unchanged. No foreign target, original vendor checkout, source or production data was removed.

The repeated native governor gate kept all23 steps, exact native arguments/test selections and the3600-second per-step timeout. Its separate evidence launcher records successful and failed captures under a new directory; the old failed/interrupted run remains intact. To avoid accumulating every CLI feature generation, the launcher performed supplemental supported own `da-cli` cleanup only after step16 and step18 passed. It checks for live native compilers, validates the dry-run plan against protected paths, performs cleanup and re-provisions the exact default runner commands from steps14/15 before the next feature plane. Each supplemental command has its own raw log/receipt; any failure or timeout stops the run instead of fabricating success. This is documented execution preparation, not an unchanged capture-only wrapper claim.

Final formatting used the explicit 12 project packages plus the standalone critic and passed without changing Rust bytes. The current-source GitNexus refresh exited0 in437.081s. Identity checks found107159 path-bearing nodes with0 ID/path mismatches,65 changed symbols with0 parity defects and both new helpers indexed. Detect-changes returned CRITICAL for23 affected processes across17 changed files, without partial/truncated result flags. Global flow coverage remains unproved:8033/8233 entrypoints were not ranked,2500 callees were skipped,22 walks hit the trace budget,534 cross-language field sites were unresolved, and the callable/directory caps and disabled FTS remain. The full native23-step gate passed in8,973.503s from the current917-file source binding; every bound source hash remained unchanged. Both supplemental boundaries completed all five commands with exit0/no timeout and unchanged protected hashes. The guard never paused; minimum free space was27.946GiB. GSD acceptance is a separate pending operation. `GOVERNOR_ARCHITECTURE_GATE_LOG_DIR` is the native log-directory variable for the retry; the earlier shorter alias was ineffective, although the separate capture launcher retained its logs. This changes a log destination only, not any gate threshold.

## Earlier GSD closeout blocker — recovered

Earlier, two supported `openExistingWorkflowDatabase` calls returned `reason: locked`; no closeout write was issued. The engine's read-only inspection returned committed schema v50 while the current engine expects v51. Its startup repair requires EXCLUSIVE access. Native `/proc/locks` inspection found read locks held by 29 live GSD processes for this project, not a write-lock owner. This explains why normal admission cannot proceed while those connections remain open.

The read-only observation shows M518 active, T01/T02/T03 pending and all 12 gate rows pending. It was observation, not workflow execution authorization. At that point no process was signalled, no database/schema row was hand-edited and no startup-repair bypass was applied. The explicitly authorized shutdown and supported migration below later recovered admission. Historical receipts remain, not a current locked-state claim.

## Authorized GSD reader shutdown and restored admission

The user authorized SIGTERM for the 29 confirmed daily-archive GSD readers. A fresh engine read-only check found no running task or other execution Attempts, and native identity checks found no child processes. All 29 SIGTERM signals were delivered through pidfds after rechecking PID start time, executable, project cwd and the exact database descriptor. They did not exit or release their locks. The user then explicitly authorized SIGKILL for the same processes; the native command rechecked their identities and absence of live children before signaling. All 29 processes terminated and native project lock inspection returned zero remaining locks. The law-nexus GSD process was preserved. No global process-name kill was used.

Supported normal engine admission then returned exit0. The engine upgraded the schema from v50 to v51 and created `.gsd/gsd.db.backup-v50`. No schema/database edit was made outside the engine. A subsequent read confirmed v51 and no running Attempts. At recovery, M517 remained complete and M518 was active with its three tasks and 12 gates pending. The later external-delivery disposition is recorded below.

Admission also printed a separate automatic Authority cutover warning: 60 historical M285–M290 statuses would change because old completion evidence is unproved. That cutover did not apply. The current fence is revision7218, AuthorityEpoch0; 139 older hierarchy rows lack lifecycle coverage, but none belong to M518. This warning is not a database-open failure and does not itself prove that M518 closeout writes are prohibited. Any M518 closeout must use current engine operations and their real refusals; the historical `db adopt --apply` and an authority bypass are not authorized here.

Temporary cleanup removed only three session-owned, reproducible source comparison/reconstruction trees:499 files,9.36 MiB. The removal manifest records every file hash. Raw logs, formatter comparison outputs, source patches/backups, frozen M517 runners/artifacts, native probes, the closed old-ledger store and current build outputs were retained. Governor hygiene was run as a dry run only; its older exec/quarantine evidence was not deleted. No product source or gate/test constant changed.

## Complete original backup — evidence correction

The final audit found that the initial aggregate `samyama-local-before.patch` and `samyama-dirty-tracked.patch` are tool-captured output, not complete patch files. Each is2,097,185 bytes and contains a `1,226,432 bytes dropped` marker; each shows165 rather than249 file boundaries. The earlier equality receipt therefore compared the same clipped output and did not prove a complete aggregate backup. Those receipts remain retained with this correction.

The original source tree is still unchanged. Every one of its247 dirty Rust files matches the raw pre-upgrade comparison-copy hashes preserved in `user-temporary-scratch-cleanup.json`. The two non-Rust changes (`Cargo.toml`, `CLAUDE.md`) match their complete, separately saved diffs. All eight original untracked files match the prebuild inventory; exact raw copies are now also retained. The checked old HEAD is unchanged.

Native `git diff --binary HEAD --output=...` now saves all249 changes without tool-output clipping. The complete3,323,584-byte patch is `/tmp/vendor-upgrade-audit/samyama-original-complete-before-upgrade.patch`, SHA256 `d79bb5221e03b2da4c4b49de8727a2d8ce35c66dd783d8c2601caf8a71146fcc`. `original-patch-full-byte-proof-correction.json` binds the raw-file hash comparison and untracked byte copies. The updated candidate patch, runtime sources and test selection did not change. Complete native file output, not aggregate clipped-output equality, is the original-patch preservation proof.

## Requirement impact for final acceptance

M518 touches R010 (portable wire contracts), R028 (independent meaningful review), R030 (source preservation), R036/R136 (command and freshness provenance), R070 (evidence before promotion), R072/R075/R118/R124 (closed graph/import and parse-automata boundaries), R074 (observable JobLedger state), R079/R081 (explicit failure/degradation) and R138 (onion dependency direction). No status is advanced or historical Python validation rerun. Detailed evidence and follow-ups are retained as `m518-requirement-impact.md` inside the evidence archive. The native 23-step gate, unchanged product-source hashes and typed UAT passed. Supported source-bound GSD validation must authorize acceptance separately. Q6 for T02/T03 remains flagged: no 10x runtime or build-capacity proof exists.

## Current native architecture result

The actual governor architecture retry passed all23 steps with exit0 and0 timeouts in8973.503s. Both before17/19 supplemental boundaries passed their five commands with unchanged protected hashes. The full917-file source binding still matches. Native gate counts are7,629 successful test executions, with repeated feature-plane tests kept distinct from878 scoped executions and4cross-generation ledger records. One adapters test was ignored; no ignored case is claimed as exercised.

Receipt: `/tmp/vendor-upgrade-audit/full-architecture-after-content-repair.json`; each step has native argv, timeout, exit, duration and stdout/stderr. The earlier17-pass/2-fail/resource-interrupted attempt remains retained. Invocation-only debug0/offline/single-job limits do not prove unrestricted-profile equivalence, MSRV1.85, performance, production readiness or10x capacity. Typed artifact/native ledger UAT below passed after this result. Source-bound supported GSD validation/closeout remains required.

## Typed UAT after the complete native gate

`gsd_uat_exec 4194da70-205e-49fe-aeda-fa86ea7b0ff7` passed the917-source/native23/supplemental/original-full-backup/frozen/closed-ledger artifact checks in3365ms. It is retained-evidence verification and opens no store. `gsd_uat_exec 47120834-3966-430e-b97d-d1d13a27cb3b` passed all four fresh current-generation native ledger contracts in519ms. Both runs exited0 with no timeout, abort, signal or lost recording and empty stderr. The current native binary has its recorded SHA; no compiler, SIGSTOP or30-second lease change was used during runtime. The original old-ledger tree remains closed and hash-identical.

The final tracked receipt bundle retains native step stdout/stderr, source hashes, bounded reviews, probe recipes, old failure/interruptions and complete original source backup. Oversized index/tree/formatter reports remain local with file hashes. Two unrelated cross-project raw process logs are excluded because command lines contained secret-like fields; no credential value is exported. The bundle is a copy of evidence, not an additional run or an independent lifecycle oracle. Runtime stores/binaries/cache and frozen M517 runner copies remain local.

No new performance, generic binary migration/downgrade, crash durability, model inference, MSRV1.85, optional Entity readiness/model enforcement/SHA-only backfill or materializer-example runtime claim follows from these checks. Successful scoped and architecture executions include repeats. Source-bound GSD validation must approve only the stated dependency compatibility scope; Q6T02/T03 stays flagged. Free-form task delivery has no Task Attempt and is recorded through external summaries and the supported cancellation/waiver route, not fabricated task completion. Historical status-changing `db adopt --apply` remains unapproved.

## Retained evidence and external execution disposition

`doc/review/vendor-upgrade-m518-evidence/manifest.json` records every member of
`raw-evidence.tar.gz` with its byte length and SHA256. The archive keeps the full
native step captures, scoped receipts, reviews, source seals, original backup,
probe recipes, failed runs and the clipping correction. The README identifies
the useful entry points and limits. Local absolute paths in raw receipts describe
the original runs; they are not promises that those paths exist on another host.
Extracting the archive does not replay a run or open a store.

The supported `gsd_skip_slice` operation cancelled the three never-dispatched
S01 tasks and recorded a slice waiver for external free-form delivery. No Task
Attempt, host Technical Verdict or task completion was fabricated. Supported
`gsd_save_gate_result` calls then saved all 12 gate dispositions. Q6/T01 is omitted
because it is a source audit; Q6/T02 and Q6/T03 remain flagged for unmeasured 10x
capacity. The other nine dispositions pass only the stated compatibility scope.
This is execution bookkeeping, not the source-bound milestone acceptance.

The native `governor --phase closeout` exited 0 but reported `drift_risk`, not a
blanket governance pass. It recorded historical control-plane/doctor findings,
stale artifacts and schema/README drift. The new `data/vendor_patches` directory
is now declared in `data/SCHEMA.yaml`, and README names M518. Existing `tools/`
schema drift and unrelated historical findings were not repaired here. The
reported `target/` size was 19.4 GB, below the unchanged 60 GB DATA cap. A supported
`governor closeout --decline` receipt defers reclamation to preserve evidence and
test binaries through validation and commit. No historical capture was deleted.

The acceptance record belongs to the GSD validation and completion operations,
which must bind the final engine `sha256:…` source aggregate after these documents
and the evidence archive are stable. The aggregate is not a git SHA. The engine
owns those records; the tracked archive does not contain a self-referential future
acceptance or commit receipt. No tracked source change is allowed between that
final seal and acceptance without obtaining a fresh seal.
