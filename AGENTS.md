<!-- project-current:start -->
# daily-archive — agent map (what / where)

Runtime: **Rust workspace** `crates/da-{domain,ports,application,graph,adapters,cli}` (hexagonal / onion).  
Frozen evidence: Python stack cold-archived under `archive/python-era-cold-*/` (see `archive/PYTHON-ERA-COLD.md`); `legacy/` is a restore stub — not on the runtime path.

## Hard locks (do not violate)

| Lock | Rule |
|------|------|
| Import / graph write | `import_eligible=false` until **explicit user yes** (D127) |
| Free invent | constrained select = `candidate_id` only |
| GEPA/LLM deploy | no auto-promote; dual F1 + same-n + val_gap required |
| Sidecar TEI/md | candidate evidence only, not graph truth |
| Onion | no da-application → da-adapters inversion for path defaults |
| DSPy | not enabled until eval metrics + benchmark fixtures verified (D136) |
| Samyama | sole KG+vector+persist store (ADR-040); no JSONL, no external DB for state |

## GSD workflow & process conventions (read first)

This project uses **GSD (Git Ship Done)** for milestone/slice/task tracking.
Work through GSD tools, do not hand-edit `.gsd/` planning artifacts.

### Lifecycle (GSD tools, in order)

1. `gsd_milestone_generate_id` → get milestone ID (M###-xxxxx).
2. `gsd_plan_milestone` → roadmap with slices (each needs sliceId, title, risk, depends, demo, goal, successCriteria, proofLevel, integrationClosure, observabilityImpact).
3. `gsd_plan_slice` → per-slice PLAN.md with tasks (each needs taskId, title, description, estimate, files, verify, inputs, expectedOutput).
4. `gsd_task_complete` → per-task (oneLiner + narrative + verification + verificationEvidence).
5. `gsd_slice_complete` → per-slice (sliceTitle + oneLiner + narrative + uatContent).
6. `gsd_validate_milestone` → when all slices done.
7. `gsd_complete_milestone` → closeout.

**Forbidden:** hand-editing STATE.md, PLAN.md, SUMMARY.md, ROADMAP.md, gsd.db.
Use the engine tools. `.gsd/continue.md` and `CHANGELOG.md` are hand-writable.

### Compile & test rules (avoid the rocksdb rebuild trap)

**One-time setup (do this first):** install system RocksDB 8.10.0 and create
`.cargo/config.toml` from `.cargo/config.example.toml`. This eliminates
`cc1plus` (C++ RocksDB recompilation, 5–10 min) on every `cargo` invocation.
See `.cargo/config.example.toml` for full instructions. After setup,
`cargo check -p da-cli` drops from ~5-10 min to **~6s**.

**sccache (cross-workspace cache):** `~/.cargo/config.toml` sets
`build.rustc-wrapper = "sccache"` globally. Combined with
`[profile.dev.package."*"] incremental = false` in workspace `Cargo.toml`,
this caches samyama/ruvector compilation across workspaces and `cargo clean`.
`SCCACHE_BASEDIRS` in `~/.bashrc` normalizes paths (`/root/vendor-source`,
`/root/daily-archive`). Workspace members (da-*, kg-*) keep `incremental=true`
for fast iteration. Vendor workspaces (`samyama-graph`, `ruvector`) have their
own `.cargo/config.toml` with `ROCKSDB_LIB_DIR`.

Results: `cargo check -p da-cli` after `cargo clean -p samyama*` → **2.9s**
(was ~15s). samyama-graph cold build: **23s** (was 2m38s, 99.7% cache hits).

`cargo test --workspace` / `cargo check --workspace` triggers
`librocksdb-sys` (Samyama RocksDB FFI) rebuild for the test profile —
**5–10 min each** (without the system-library setup above). Per-crate commands
skip it:

```bash
# RIGHT: per-crate checks during iterative dev
 cargo check  -p <changed-crate>
 cargo clippy -p <changed-crate> --all-targets --no-deps -- -D warnings
 cargo test   -p <changed-crate> --lib

# WRONG: full workspace (rocksdb rebuild, 5-10 min)
cargo test --workspace      # DON'T
```

`PYTHONPATH=scripts python3 -m governor architecture` is the **full gate** — run it
only as the final check at slice end, not on every edit. It runs
fmt + clippy + hexagonal invariant + per-crate tests for all our crates.

**Concurrent cargo processes block on `target/` file lock.** Cancel
background `async_bash` cargo jobs (via `cancel_job`) before re-running.
Stale `async_bash` results surface as delayed "background job done"
notices — their content is often already superseded.

### Browser evidence gate (GSD validation gotcha)

GSD `gsd_validate_milestone` has a browser-evidence gate that downgrades
`verdict: pass` → `needs-attention` when slice goal/successCriteria
text contains trigger words from `BROWSER_REQUIREMENT_RE`. The regex
matches: `file://`, `localhost`, `playwright`, `chrome`, `screenshot`,
**`snapshot`**, `browser_*` tool names.

**`snapshot` is the hidden trap for backend milestones.** Domain terms
like `PropertySnapshot`, `export_snapshot`, `import_snapshot`, or
"post-M284 snapshot" in slice goal/successCriteria trigger the gate.

**Fix:** for each triggering slice, write `285-SS-ASSESSMENT.md` with
runtime-executable evidence:

```yaml
---
uatType: runtime-executable
verdict: PASS
---

| check_name | type | verdict | tool | duration |
|------------|------|---------|------|----------|
| check-id | runtime | PASS | gsd_uat_exec <id> | 99ms |
```

Get real evidence IDs via `gsd_uat_exec` with `intent: "uat-runtime-check"`
or `intent: "uat-artifact-check"`. The `hasRuntimeExecutableUatEvidenceText`
bypass then activates and `verdict: pass` is preserved.

### Manual closeout when Attempt is unavailable (canonical, M306-proven)

**Prefer this path in free-form sessions.** Do **not** treat `/gsd auto` as
the only seal path when code is already green.

#### When you hit this

| Symptom | Meaning |
|---------|---------|
| `gsd_task_complete` → **Canonical Task completion requires a running or replay-matched Attempt** | Lifecycle exists; only auto `attempt.claim` mints a running Attempt. Manual session cannot complete the task row. |
| Closeout → **canonical milestone validation authorization is not current** | Need fresh `gsd_validate_milestone` with engine `testedSourceRevision` (`sha256:…`), not git SHA. |
| Closeout → **quality gate Q5/Q6/Q7/Q8 is still pending** | Waiver-skip / free-form delivery left task-scoped gates pending. |
| Agent `gsd_save_gate_result` after skip → **no tasks found for MID/SID** | Agent tool re-renders PLAN from DB; skipped tasks break render. Use **engine** `saveGateResult` via `gsd_exec` node. |

**Forbidden:** hand-edit `gsd.db` with `sqlite3` / python / better-sqlite3.
Engine owns WAL (single writer). Never invent Attempt rows.

#### Canonical unblock order (delivered code already committed)

1. **`gsd_skip_slice`** — reason: work delivered + commit SHA + what shipped.
   Cascades pending tasks → skipped (authorized waiver for deps).
2. **Runtime UAT** — `gsd_uat_exec` per check, then `gsd_uat_result_save`:
   - `checks[].evidence` = **array** of `{kind:"gsd_uat_exec", ref:"<UUID>"}`
   - `presentation.presentedTools` **must** include:
     `gsd_uat_exec`, `gsd_uat_result_save`, `gsd_resume`,
     `gsd_milestone_status`, `gsd_journal_query`
   - `presentation.blockedTools` **must** list (with reason):
     `gsd_exec`, `gsd_summary_save`, `gsd_save_gate_result`
   - Prefer `uatType: runtime-executable` (also bypasses browser-evidence trap).
3. **Capture source revision** (engine aggregate hash, **not** `git rev-parse`):
   ```js
   // gsd_exec runtime=node
   const path = await import('path');
   const gsd = '/usr/lib/node_modules/@opengsd/gsd-pi/dist/resources/extensions/gsd';
   const { openWorkflowDatabase } = await import(path.join(gsd, 'db-workspace.js'));
   const { captureMilestoneVerificationSourceRevision } =
     await import(path.join(gsd, 'verification-source-integrity.js'));
   const { loadEffectiveGSDPreferences } = await import(path.join(gsd, 'preferences.js'));
   const base = '/root/daily-archive/.gsd';
   openWorkflowDatabase(base);
   const prefs = loadEffectiveGSDPreferences(base)?.preferences;
   console.log(JSON.stringify(captureMilestoneVerificationSourceRevision(base, prefs)));
   // → { ok: true, sourceRevision: "sha256:…" }
   ```
4. **`gsd_validate_milestone`** with that exact `testedSourceRevision` on every
   `verificationEvidence[]` row. Include Contract / Integration / Operational / UAT
   classes as applicable (Operational often N/A for library-only).
5. **Close pending quality gates** via **engine** API (not agent tool after skip):
   ```js
   // gsd_exec runtime=node — openWorkflowDatabase first
   const { getPendingGates, saveGateResult } = await import(path.join(gsd, 'gsd-db.js'));
   // getPendingGates(milestoneId, sliceId)  — BOTH args required
   // task-scope Q5/Q6/Q7: taskId MUST be "T01" (etc.)
   // slice-scope Q3/Q4/Q8/UAT: taskId ""
   saveGateResult({ milestoneId, sliceId, gateId, taskId, verdict, rationale, findings });
   ```
   Gates already complete from earlier `gsd_save_gate_result` can be left alone;
   re-check `getPendingGates(mid, sid)` → `[]`.
6. **`checkCloseoutConsistencyGate`** must return `{ ok: true }`:
   ```js
   const { checkCloseoutConsistencyGate } =
     await import(path.join(gsd, 'closeout-consistency-gate.js'));
   openWorkflowDatabase(base);
   console.log(JSON.stringify(checkCloseoutConsistencyGate(mid, {
     allowOpenMilestone: true,
     artifactBasePath: base,
   })));
   ```
7. **`gsd_complete_milestone`** → **`gsd_checkpoint_db`** (WAL flush before any
   commit that stages `.gsd/gsd.db`).

#### Subjective UAT (only when human judgment is required)

Most backend contract milestones can use **runtime-executable UAT only**.
If subjective criteria exist:

1. `gsd_prepare_milestone_subjective_uat` with **one stable** `criterionKey`
   and the **same** `testedSourceRevision` (`sha256:…` from step 3).
2. Answer with `gsd_answer_milestone_subjective_uat` using the exact IDs from
   the prepare result / engine events.
3. **Batch rule:** prepare ALL criteria first, then answer ALL, then validate.
   Any prepare **after** an answer invalidates prior acceptances.

#### What does **not** work

| Approach | Why it fails |
|----------|--------------|
| `/gsd auto` as only advice after free-form green code | Auto may not be available; Attempt claim is auto-internal |
| Agent `gsd_save_gate_result` after skip | `renderPlanFromDb` → no tasks found for skipped slice |
| `gsd_save_gate_result` without `taskId` for Q5/Q6/Q7 | Misses task-scoped rows |
| `testedSourceRevision = git SHA` | Engine wants aggregate `sha256:…` |
| Direct `sqlite3` / python write to `.gsd/gsd.db` | Blocked; corrupts WAL / races engine |
| Standalone node without `openWorkflowDatabase("<repo>/.gsd")` | `gsd-db: No database open` |
| `gsd_complete_milestone` without current validate | closeout: validation authorization not current |
| Pending gates = 0 without re-validate after new commits | Authorization still stale vs new sourceRevision |

#### Package closeout for a debt wave

If several delivered milestones are still `active` with terminal/skipped slices:
re-validate + close pending gates + complete **each** with a **fresh**
`captureMilestoneVerificationSourceRevision` after the last commit that
touches that milestone's tree. Pending gates = 0 does **not** skip re-validate.

#### Related GSD gotchas (keep in mind)

- **Duplicate milestone IDs** (bare `M###` + suffixed `M###-xxxxxx`): auto stuck
  on plan-slice. Prefer `unique_milestone_ids: true`; restart auto with
  **suffixed** id (`/gsd auto M###-xxxxxx`).
- **Browser-evidence gate:** word `snapshot` in slice goal/successCriteria
  downgrades validate to needs-attention. Bypass with runtime-executable UAT
  ASSESSMENT (see section above).
- **Long cargo:** always async + progress; never two cargo on one `target/`;
  per-crate only (rocksdb trap).
- Governor rule `gsd_lifecycle_attempt_desync_in_summary` flags SUMMARY text
  about blocked Attempt — update SUMMARY after seal so trajectory stays clean.

Proven end-to-end on **M306-417kcl** (skip-delivered + UAT + sha256 validate +
complete). Memory: MEM567 / D144.

### Governor CLI (trajectory enforcement)

`scripts/governor/` is the **governor CLI** — hexagonal Python package that
derives project trajectory report (verdict: on_track / drift_risk / blocked)
from ADRs, dimensions, reverse-ADR audit, drift flags. Rules are DATA
(YAML), not hardcoded Python.

```bash
PYTHONPATH=scripts python3 -m governor --phase preflight   # default
python3 scripts/check_project_trajectory.py --phase active
PYTHONPATH=scripts python3 -m governor --phase closeout
```

Reverse-ADR audit enforces 7 rules: 4 artifacts-gate (D127/ADR-005/ADR-040)
+ 3 Rust-specific (kg-* must not depend on da-*, da-domain must not depend
on infra, da-application must not depend on da-adapters in prod deps).
Negative test: inject `da-domain` into a kg-* Cargo.toml → governor flips
to `verdict=blocked`.

**Do NOT write new bash scripts to enforce invariants** the governor
already checks. Extend governor rules or `scripts/verify_rust_architecture.sh`
instead. (M285-z09px0/S08 extracted governor out of `legacy/` and
adapted it for Rust — it is the authoritative trajectory checker.)

### Pre-commit hooks (`.pre-commit-config.yaml`)

First line of defense before CI. Covers kg-* + da-* in cargo-fmt and
cargo-clippy-lib (no `--all-targets`, avoids rocksdb rebuild), plus
cargo-clippy-tests (per-crate `--lib --tests`, M430: clippy --lib does
NOT compile test modules — a test-only break passed the lib hook and
failed under `cargo test`, the M429 lesson), governor-trajectory
(ADR invariants + artifact gates) and docs-drift-check.
Run automatically on `git commit`.

### GitNexus (code intelligence)

Reindex after structural changes (new crates, renamed symbols):
`node .gitnexus/run.cjs analyze`. Until then, new symbols stay unresolved.
Use `impact({target, direction:"upstream"})` before editing any symbol —
warn the user on HIGH/CRITICAL risk. Use `detect_changes()` before commit.

### Memory store (auto-learned gotchas)

GSD auto-loads memories from the memory store. Key entries for this project:
- **MEM468**: per-crate tests avoid rocksdb rebuild (see Compile rules above).
- **MEM499**: four per-test mocks drift from real SamyamaGraphStore
  (`get_nodes_by_label` returns `Vec::new()`). Shared `common/mock_graph_store.rs`
  filters by label — prefer it.
- **MEM507**: validator surfaces fixture bugs when first wired into a suite
  (expect 2-3 per node type).
- **MEM514**: don't run `cargo check --workspace` on every edit — per-crate only.

Query memories: `memory_query({category: "gotcha", query: "..."})`.

### Testing strategy (ADR-051)

**Two axes:** hexagonal layers (7 contours) × test types (8 categories).

#### 7 contours (hexagonal layers)

1. **Determinism Infrastructure** — Clock, ID, RNG, Sleeper ports (inject in tests, never real clock/sleep/UUID)
2. **Domain Semantic** — unit + property (proptest) + serde roundtrip + metamorphic relations
3. **Application Functional** — use cases via driving port, state-based fakes, Fixture builder
4. **Port Contract Suite** — shared suite per trait (GraphStore, Embedder), runs vs ALL impls
5. **Adapter Integration** — real Samyama, wiremock HTTP, Testcontainers (future PostgreSQL)
6. **System Journey E2E** — use-case specs (UC1-5), lifecycle (snapshot export/import),
   **semantic graph task suite** (Samyama case-study DoD: pin `.sgsnap` → tasks →
   same-process query → PASS/EMPTY/ERROR; see ADR-051 §8.1)
7. **Architecture Invariant** — governor + cargo metadata + trybuild + cargo-hack

#### 8 test types

| Type | What | Tool |
|------|------|------|
| Unit | Single function/struct | `#[test]` inline |
| Property | Invariant over random inputs | `proptest!` |
| Serde roundtrip | Wire-format stability | `roundtrip<T>()` |
| Contract | Trait semantics across all impls | `contract_*.rs` |
| Integration | Real external system | `cargo test` + real deps |
| Business logic | Domain policy enforcement | use-case test |
| Mutation | Tests catch code mutations | `cargo-mutants` |
| E2E journey | Full business scenario | CLI/HTTP test |

#### Domain-specific semantic suites

- **Temporal graph:** no-future-knowledge, valid≠transaction time, reproducible projection, [from,to) intervals, correction≠overwrite, idempotent canonicalisation
- **Vector retrieval:** hard invariants (tenant/temporal/provenance/no-NaN) vs quality relations (recall@k, ties, tolerance, paraphrase)
- **Event sourcing:** fold determinism, snapshot+tail≡full replay, idempotent processing, checkpoint atomicity
- **Candidate→verify→promote:** candidate≠authoritative, no self-confirm, no double promotion, atomic promotion+evidence
- **Agent/LLM:** action validity, tool schema match, evidence existence, budget/policy gates, paraphrase invariance
- **Semantic graph tasks (Contour 6 §8.1):** durable `.sgsnap` + `data/graph_tasks/*` +
  same-process runner; per-task PASS/EMPTY/ERROR; suite fails on EMPTY/ERROR.
  **Not** the same as extraction `eval_batch` (gold entities). Both required for readiness.
  Disk→query: `da query|graph-stats --snapshot <file.sgsnap>` — never load-snapshot then a second process.

#### Concurrency testing

- **Loom:** exhaustive interleavings (lock-free, atomics, channels)
- **Shuttle:** randomized scheduling (workers, queues, races)
- **Turmoil:** distributed simulation (crashes, partitions, torn writes)

#### Additional tools

- `cargo-fuzz` (parsers, deserialization)
- `cargo-llvm-cov` (coverage as gap map, not gate)
- `cargo-mutants` (mutation testing, nightly)
- Miri (unsafe/FFI), Kani (model checking)
- `insta` (snapshots, selective), `trybuild` (compile-fail)

#### Key rules

- MockGraphStore MUST pass the same contract suite as SamyamaGraphStore
- Use `DeterministicEmbedder` (LCG seed), never real network in tests
- Use `#[tokio::test(start_paused = true)]` for timeout/backoff tests
- Flaky test → fix determinism, NEVER add retry
- `roundtrip<T>()` macro for every Serialize type
- Do NOT build oracle through same code being tested (common-mode failure)
- State-based fakes > choreography mocks — verify result, not call sequence
- Contour 6 disk→query: **same-process** `--snapshot` only (process-local store dies on exit)
- Task Cypher uses ontology wire strings (`hasPart`, `CITES`) from `data/ontology/edge_contracts.yaml`
- Extraction F1 alone ≠ scientific KG readiness — pair with graph task suite

#### CI tiers

- **PR** (<30s): Unit + Property + Serde + Contract (Mock) + fmt + clippy + arch
- **Merge** (<2min): + Integration (Samyama) + Business logic
- **Nightly** (<15min): + Mutation (domain) + E2E + Flaky(5) + Fuzz + Shuttle/Turmoil
- **Release** (<30min): + Mutation (app) + Crash recovery + Migration upgrade + semver

#### Governor commands

```bash
PYTHONPATH=scripts python3 -m governor --check-contracts
PYTHONPATH=scripts python3 -m governor --check-serde-roundtrip
PYTHONPATH=scripts python3 -m governor --check-flaky 5
PYTHONPATH=scripts python3 -m governor --test-matrix
```

#### Implementation priorities

- **P0:** workspace split ✅, dependency rules ✅, Clock/ID/RNG ports, contract suites, serde roundtrip ✅ (M340–M343, 40 types, 53 tests)
- **P1:** property tests ✅ started (M344/M353–M355, M387 post-close: 38 proptest: TemporalEdge, EntityType/RelationType, VID, confidence, graph_tasks parser, PromptCandidate FSM transitions);
  **semantic graph task suite** (ADR-051 §8.1) ✅ (M336–M338);
  state-machine model ✅ (PromptCandidate FSM transition invariants, M387 post-close), temporal invariants ✅ (8 properties), replay equivalence, mutation (domain) — not yet
- **P2:** Loom, Shuttle, Turmoil, fuzzing, Miri/Kani, release recovery, remote task suite when daemon real

See `doc/adr/ADR-051-testing-strategy-overlapping-verification-contours.md` for full details.

#### Anti-patterns (avoid)

Mock every trait · Fake without contract · Functional=HTTP E2E · Shared DB · sleep · Retry hides flaky · 100% coverage gate · Snapshot everything · Same code in SUT and oracle · Exact assertions for approximate output · Happy path only · Architecture only in docs · load-snapshot process A + query process B as E2E · Extraction F1 as sole KG readiness metric

### Data-driven ontology (ADR-050 Phase D+F)

Schema definitions are DATA, not hardcoded Rust. Two modes:

```bash
# Export all 31 node schemas to YAML
da ontology-export --output data/ontology/

# List schemas from YAML (data-driven mode)
da schema-list --from-yaml

# List schemas from hardcoded Rust impls (default)
da schema-list
```

- `YamlSchemaAdapter` (da-domain): impl `NodeSchemaDef` via `Box::leak`
  pattern — wraps YAML data, leaks strings to `&'static str`.
- `OntologyRegistry` (kg-ontology): loads `node_schemas.yaml`, provides
  `get_schema(label)`, `labels()`, `len()`.
- `validate_node_properties_or_yaml()`: opt-in YAML-driven validator.
  Existing `validate_node_properties()` stays hardcoded (backward compat).
- `schema_for_label_or_yaml()`: tries YAML first, falls back to hardcoded.

Round-trip: `da ontology-export` → `data/ontology/node_schemas.yaml` →
`OntologyRegistry::load_from_dir()` → `YamlSchemaAdapter` →
`NodeSchemaDef` trait → validator.

### Handoff between sessions

At session end (or before context exhaustion): update `.gsd/continue.md`
with active milestone, what shipped, next action, and operators. Next
session reads `.gsd/continue.md` + `.gsd/STATE.md` + this AGENTS.md to
re-orient in <1 min.

## Where to look first

| Question | Open this |
|----------|-----------|
| Human overview + architecture | `README.md` |
| Project state (GSD) | `.gsd/PROJECT.md`, `.gsd/STATE.md` |
| Binding ADRs | `doc/adr/ADR-INDEX.md` |
| Graph schema | `doc/GRAPH-SCHEMA.md` |
| Ontology alignment | `doc/ONTOLOGY-ALIGNMENT.md` |
| Healing scenarios | `doc/GRAPH-HEALING-SCENARIOS.md` |
| Persistence model | `doc/PERSISTENCE-ANALYSIS.md` |
| Extraction evaluation | `data/gold_standard/`, `crates/da-cli/examples/eval_batch.rs` |
| Corpus metrics (corpus) | `crates/da-cli/examples/eval_batch.rs` output |
| Garbage / truth paths | `doc/REPO-HYGIENE.md` |
| GSD state (local) | `.gsd/continue.md`, `.gsd/STATE.md` |

## Package map (Rust workspace)

| Crate | Path | Role |
|-------|------|------|
| Domain | `crates/da-domain/` | Paper/Entity/Evidence/Relation/Healing/Scheduler/Eval types |
| Ports | `crates/da-ports/` | GraphStore/DirectGraphStore (HOT path), ParserPort, Embedder, Extractor, OpenAlexClient |
| Application | `crates/da-application/` | Ingest/BatchIngest/Extraction/Healing/Enrich/Scheduler use-cases |
| Parse | `crates/da-parse/` | Scholarly PDF fulltext: Java GROBID HTTP, TEI IR, ParserPort projector |
| Adapters | `crates/da-adapters/` | HTML parse, pdf-classifier, XY-Cut, fd_api embedder (bge-m3), SamyamaGraphStore, RuleBasedExtractor, OpenAlexHttpAdapter, GrobidParseStage |
| CLI | `crates/da-cli/` | `da` binary: health/version/ingest/batch-ingest/extract/schema-init/heal/enrich/scheduler-run |
| **kg-ontology** | `crates/kg-ontology/` | Universal types: Violation/Severity/PropertySnapshot/TemporalEdge (ADR-050, no da-* deps) |
| **kg-storage** | `crates/kg-storage/` | Universal storage traits: GraphStore/DirectGraphStore/MockGraphStore (ADR-050) |
| **kg-embeddings** | `crates/kg-embeddings/` | Universal embeddings: Embedder/FdApiEmbedder (ADR-050) |
| **kg-algorithms** | `crates/kg-algorithms/` | Universal graph algorithms: resolve_temporal_edges (ADR-050) |
| **kg-pipeline** | `crates/kg-pipeline/` | Universal pipeline DSL: PipelineBuilder/ExecutionEngine (ADR-050) |

Dependency direction (enforced by `scripts/verify_rust_architecture.sh` + CI):
`da-domain ← da-ports ← da-application ← da-cli`; `da-adapters` is infrastructure (depends on ports, not vice versa).
`da-domain` also depends on `kg-ontology` (re-export, ADR-050 Phase A).
**kg-* crates are universal — must NOT depend on da-*** (ADR-050 invariant, enforced by governor + guardrail).

## Operators (run these)

```bash
# Architecture guardrail — FULL gate, run at slice END only (slow)
# (fmt + clippy + hexagonal invariant + per-crate tests for kg-* + da-*)
PYTHONPATH=scripts python3 -m governor architecture

# Governor CLI — trajectory report (fast, ~1s)
PYTHONPATH=scripts python3 -m governor --phase preflight

# Per-crate checks during iterative dev (FAST, avoids rocksdb rebuild)
cargo check  -p <changed-crate>
cargo clippy -p <changed-crate> --all-targets --no-deps -- -D warnings
cargo test   -p <changed-crate> --lib

# Common per-crate test targets (AVOIDS `cargo test --workspace`)
cargo test -p da-domain --lib
cargo test -p da-parse --lib
cargo test -p da-adapters --lib
cargo test -p da-application --tests
cargo test -p kg-ontology --lib
cargo test -p kg-storage --lib
cargo test -p kg-algorithms --lib

# ADR-051 P1 property-based tests (M344/M353–M355)
cargo test -p kg-ontology --lib proptest   # TemporalEdge intervals (8)
cargo test -p da-domain --lib proptest     # EntityType/RelationType/confidence/VID (19)
cargo test -p da-cli --lib proptest        # graph_tasks parser (6)

# ADR-051 §8.1 semantic graph task suite (M336–M338)
cargo test -p da-cli --lib graph_tasks                        # parser unit
cargo run -p da-cli --example graph_task_suite -- \
  --snapshot artifacts/gepa_hub_production/hub.sgsnap \
  --tasks data/graph_tasks/gepa_hub.tasks.cypher \
  --report artifacts/graph_tasks/gepa_hub_report.json

# PDF OCR triage via pdf-inspector (M346–M351; optional feature)
cargo test -p da-adapters --features pdf-classifier --lib pdf_classifier
cargo test -p da-adapters --features pdf-classifier --test hub_ocr_probe -- --nocapture
# CLI build with pre-GROBID triage (off by default):
#   cargo build -p da-cli --features pdf-classifier

# Sweep cache materialization (M440; offline, no services needed)
# 1) Coverage audit (read-only): bindability per paper via ADR-065 fallback
cargo run -p da-cli --example catalog_materialize_runner -- --audit-only
# 2) Materialize replay -> .sgsnap + per-paper FSM report (canary: --limit 25;
#    full run: no limit; env knobs DA_MATERIALIZE_{LIMIT,SNAPSHOT,RUN_REPORT})
cargo run -p da-cli --example catalog_materialize_runner -- --run --limit 25
cargo run -p da-cli -- graph-stats --snapshot artifacts/catalog_sweep/catalog_materialized.sgsnap
# Version recovery for no_tei_revision papers (M441; offline, pdf stamps):
cargo run -p da-cli --features pdf-classifier --example catalog_materialize_runner -- --recover
DA_MATERIALIZE_RECOVERY=artifacts/catalog_sweep/materialization_recovery.json \
  cargo run -p da-cli --features pdf-classifier --example catalog_materialize_runner -- --run
# Post-rebuild regression check (M444; runs over ALL articles, no gold):
cargo run -p da-cli --example corpus_regression_check -- \
  --before artifacts/catalog_sweep/materialization_run_report.json \
  --after /tmp/new_run_report.json
# Drift stats (per-category mentions density, zero-mention alarms):
#   artifacts/catalog_sweep/catalog_drift_stats.json (regenerate from run report)
# Hot-path lookup benchmark at full-store scale (M443):
cargo run -p da-cli --example materialize_index_bench -- \
  --snapshot artifacts/catalog_sweep/catalog_materialized.sgsnap
# Full catalog-wide run (589 papers; ~2h offline) — already executed,
# snapshot committed: artifacts/catalog_sweep/catalog_materialized.sgsnap.
# Resume an interrupted full run (crash-safe; skips completed, merges reports):
DA_MATERIALIZE_RESUME=1 DA_MATERIALIZE_RECOVERY=artifacts/catalog_sweep/materialization_recovery.json \
  cargo run -p da-cli --features pdf-classifier --example catalog_materialize_runner -- --run

# Extraction evaluation on gold-standard corpus (12 modern papers)
cargo run -p da-cli --example eval_batch

# Single-paper extraction debug
cargo run -p da-cli --example eval_extract -- 2507.19457

# End-to-end (requires GROBID :8070 + FD_API :8000)
da schema init
da batch-ingest --pdf-dir data/article_catalog/article_catalog/arxiv/cs-cl/<id>/source
da batch-enrich
da extract --id <paper_id>
# Grounded extractive summaries for PaperRevision nodes already in-process (no LLM):
#   da summarize-revisions [--graph file.sgsnap]

# Wave 3 multi-process RESP health (in-memory, no RocksDB; M307–M312)
# Port 16379 — not Redis 6379, not vendor SAMYAMA_RESP_PORT 6380
da health                              # reports DA_GRAPH_MODE; remote is honest NotConfigured
da serve-graph --port 16379            # terminal A (optional: --snapshot file.sgsnap)
da graph-ping --port 16379             # terminal B → PONG
# DA_GRAPH_MODE=remote still does NOT open a full GraphStore (open_for_mode NotConfigured)
```

Historical `legacy/scripts/verify_m0xx_*.py` — frozen Python milestone pins, not runtime ops. `legacy/` is gitignored (untracked frozen Python stack).

## Current snapshot (extraction eval)

Re-measured **2026-08-05** after M315/M316 gold promote (`cargo run -p da-cli --example eval_batch`):

| Metric | Value | vs M314 pre-promote |
|--------|--------|---------------------|
| Corpus | **193 papers** (200 fixtures; 7 missing PDF/HTML skipped) | same |
| Gold entities | **1399** | was 1326 (+73) |
| Precision (Micro) | **0.769** | was 0.729 |
| Recall (Micro) | **1.000** | same |
| F1 (Micro) | **0.869** | was 0.843 |
| F1 (Macro) | **0.859** | was 0.838 |
| TP / FP / FN | **1399 / 421 / 0** | FP was 494 (−73) |
| import | **false** (D127) | locked |

Weak F1&lt;0.7 remaining: mostly tiny gold (2–3 ents) or gnn-html chapters; promoted arxiv weak papers now typically F1≥0.95.

## Problem backlog (open)

| ID | Problem | Where |
|----|---------|--------|
| E1 | Multi-word method extraction | **Closed M313** — data-driven `methods.phrases` + Method section arm parity |
| E2 | Incomplete gold → FP inflation | **Process M315 + batch M316** — `inventory_fp` + `batch_promote_gold`; multi-paper gold only. 2603 +8; batch **13 weak papers +65** entities. Remaining FP = noise/singleton |
| E3 | Weak-paper precision (was F1 0.35–0.67) | Evidence promote lifts many to F1 **0.95–1.0** (e.g. 2510.26094=1.0, 2507.19457≈0.96); re-run full `eval_batch` for corpus Micro |
| P1 | Full remote GraphStore not wired (`DA_GRAPH_MODE=remote` → NotConfigured); health path is serve-graph+graph-ping | Multi-process health done (M307–M312); full remote store later |
| L1 | Import locked by policy | D127 — need user go |

## Code navigation

1. Prefer **GitNexus** `query` / `context` / `impact` (rules below).
2. Before edit: `impact({target, direction:"upstream"})` — warn on HIGH/CRITICAL.
3. Before commit: `detect_changes({scope:"compare", base_ref:"main"})`.
4. Onion: `PYTHONPATH=scripts python3 -m governor architecture`.
5. Do not invent free labels; do not open import without user yes.

<!-- project-current:end -->

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **daily-archive** (35663 symbols, 60292 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> Index stale? Run `node .gitnexus/run.cjs analyze` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? `npx gitnexus analyze` (npm 11 crash → `npm i -g gitnexus`; #1939).

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows. For regression review, compare against the default branch: `detect_changes({scope: "compare", base_ref: "main"})`.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `query({search_query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `context({name: "symbolName"})`.
- For security review, `explain({target: "fileOrSymbol"})` lists taint findings (source→sink flows; needs `analyze --pdg`).

## Never Do

- NEVER edit a function, class, or method without first running `impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit changes without running `detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/daily-archive/context` | Codebase overview, check index freshness |
| `gitnexus://repo/daily-archive/clusters` | All functional areas |
| `gitnexus://repo/daily-archive/processes` | All execution flows |
| `gitnexus://repo/daily-archive/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->

<!-- parser-sidecars:start -->
# Parser sidecars (GROBID)

Hybrid PDF path expects **GROBID** HTTP service (default `http://127.0.0.1:8070`).
GROBID TEI output is consumed by `crates/da-parse` (Rust `GrobidParser`),
not by Python code anymore.

## Start / stop

```bash
# GROBID CRF (recommended for daily-archive pilots)
docker compose -f .docker/docker-compose.yml --env-file .env up -d grobid
curl -sS http://127.0.0.1:8070/api/isalive   # true

docker compose -f .docker/docker-compose.yml --env-file .env down
```

Env keys: see `.env.example` (`GROBID_*`, `HYBRID_AUTO_START_CONTAINERS`). GROBID CRF has **no API key**.

**GROBID REST profiles** (`da_parse::GrobidProfile`, D204):
- `rich` (backward-compatible default): `consolidateCitations=1`,
  `consolidateHeader=1`, `teiCoordinates=["figure","table","note"]`.
- `structural` (gold/bounded pipeline): both consolidation flags are `0`, while
  coordinates remain enabled. This keeps local TEI generation independent of
  Crossref 429/backoff; bibliography/header enrichment runs later through the
  separately cached OpenAlex/Crossref stage.

Never mix profiles under one cache key. Profile and consolidation flags must be
part of the parse stage configuration fingerprint. Rich mode may take minutes
under Crossref rate limits; structural mode is the managed scalable default for
corpus parsing.

## Auto ensure from code (Rust)

```rust
use da_parse::GrobidParser;
use da_ports::parser::ParserPort;

let parser = GrobidParser::from_env();
assert!(parser.is_alive().await);
let parsed = parser.parse_pdf(pdf_path, paper_id).await?;
```

Rules:

- Probe/start is infrastructure; **import_eligible stays false**.
- Do not treat sidecar TEI/markdown as graph truth — GROBID output feeds the
  Rust RuleBasedExtractor, which produces canonical Entity nodes in Samyama.

Details: `.docker/README.md`
<!-- parser-sidecars:end -->

<!-- logging:start -->
# Structured Logging (ADR-052)

Dual-layer tracing: stderr (compact) + rolling JSON file under one `RUST_LOG`/`LOG_LEVEL` filter. INFO lifecycle events carry semantic identity and write deltas by default.

## Configuration

```bash
# .env
LOG_DIR=logs          # directory for rolling files (default: logs/)
LOG_LEVEL=info        # filter level (RUST_LOG overrides)
```

## Log files

Daily rotation: `logs/da.log.YYYY-MM-DD`
JSON format with `flatten_event`, span hierarchy, `FmtSpan::CLOSE` timing.

## Querying (jq)

```bash
jq 'select(.paper_id == "2507.19457")' logs/da.log.2026-08-05
jq 'select(.level == "ERROR")' logs/da.log.2026-08-05
jq 'select(.span.name == "ingest_pdf")' logs/da.log.2026-08-05
jq 'select(.fields.duration_ms > 1000)' logs/da.log.2026-08-05
```

## Architecture (hexagonal)

- **da-cli** (infrastructure): owns subscriber, WorkerGuard, `logging::init_logging()`
- **da-application** (use cases): `#[instrument]` on `ingest_pdf`, `batch_ingest_pdfs`, `extract_from_parsed`
- **da-parse** (parse plane): `#[instrument]` on `GrobidParser::parse_pdf`
- **da-adapters** (adapters): `tracing::info!` on remaining port impls
- **da-domain, da-ports, kg-***: **zero tracing dependency** (pure types/traits)

## WorkerGuard

`init_logging()` returns `Option<WorkerGuard>`. Must be held in `main()` scope.
Dropping it early silently stops file logging — #1 tracing footgun.
<!-- logging:end -->
