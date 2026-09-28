<!-- project-current:start -->
# daily-archive — agent map (what / where)

Runtime: **Rust workspace** `crates/da-{domain,ports,application,parse,adapters,cli,mcp}` (hexagonal / onion).  
Frozen evidence: Python stack cold-archived under `archive/python-era-cold-*/` (see `archive/PYTHON-ERA-COLD.md`); `legacy/` is a restore stub — not on the runtime path.

## Hard locks (do not violate)

| Lock | Rule |
|------|------|
| Import / graph write | `import_eligible=false` until **explicit user yes** (D127). ADR-076 §14 adds `deployed=false`, `auto_promote=forbidden`, `graph_write=forbidden` |
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
See `.cargo/config.example.toml` for full instructions.

**sccache (cross-workspace cache):** `~/.cargo/config.toml` sets
`build.rustc-wrapper = "sccache"` globally. Combined with
`[profile.dev.package."*"] incremental = false` in workspace `Cargo.toml`,
this caches samyama/ruvector compilation across workspaces and `cargo clean`.
`SCCACHE_BASEDIRS` in `~/.bashrc` normalizes paths (`/root/vendor-source`,
`/root/daily-archive`). Workspace members (da-*, kg-*) keep `incremental=true`
for fast iteration. Vendor workspaces (`samyama-graph`, `ruvector`) have their
own `.cargo/config.toml` with `ROCKSDB_LIB_DIR`.

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
"post-migration snapshot" in slice goal/successCriteria trigger the gate.

**Fix:** for each triggering slice, write `<MM>-<SS>-ASSESSMENT.md`
(e.g. `512-02-ASSESSMENT.md`) with runtime-executable evidence:

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

### Manual closeout when Attempt is unavailable

Free-form sessions cannot mint a running Attempt, so `gsd_task_complete` fails there. Use engine calls, never a hand-written row: `testedSourceRevision` is the engine aggregate `sha256:…` from `captureMilestoneVerificationSourceRevision`, not a git SHA; `gsd_save_gate_result` after `gsd_skip_slice` finds no tasks; Q5/Q6/Q7 need a real taskId; never write `.gsd/gsd.db` with sqlite3 or python (engine owns the WAL). The worked sequence (MEM567 / D144) is recorded in `git log -p -- AGENTS.md`.

### Governor CLI (trajectory enforcement)

`scripts/governor/` is the **governor CLI** — hexagonal Python package that
derives project trajectory report (verdict: on_track / drift_risk / blocked)
from ADRs, dimensions, reverse-ADR audit, drift flags. Rules are DATA
(YAML), not hardcoded Python.

```bash
PYTHONPATH=scripts python3 -m governor --phase preflight   # default
python3 scripts/check_project_trajectory.py --phase active
PYTHONPATH=scripts python3 -m governor --phase closeout
PYTHONPATH=scripts python3 -m governor hygiene             # dry run; --apply to prune
```

Reverse-ADR audit enforces 7 rules: 4 artifacts-gate (D127/ADR-005/ADR-040)
+ 3 Rust-specific (kg-* must not depend on da-*, da-domain must not depend
on infra, da-application must not depend on da-adapters in prod deps).
Negative test: inject `da-domain` into a kg-* Cargo.toml → governor flips
to `verdict=blocked`.

`no_new_product_python_scripts` (D237) forbids new Python under `scripts/`
except the governance surface. Two allowlisted roots exist: `scripts/governor/`
and `scripts/tests/`. A new tool elsewhere under `scripts/` is a hard finding —
put it in `scripts/governor/` (or `checks/*.yaml` as DATA) instead.

#### Control-plane health (dimension `gsd_control_plane`)

The 2026-09-20..27 journal review found every recurring failure was control-plane
rather than product: projection-claim contention, liveness wedges, lease
conflicts, Technical-Verdict blocks, an always-red doctor, and unbounded runtime
stores. The trajectory report now measures that class:

- thresholds, the retention policy and the closed signal vocabulary live in
  `scripts/governor/checks/gsd_control_plane.yaml` (DATA). Tightening a limit is an
  edit there, not a Python change.
- accepted historical doctor divergence lives in
  `scripts/governor/checks/gsd_doctor_baseline.json`. Suppression is scoped by
  milestone, so a *new* occurrence of a baselined code in live work still fires.
  Read and extend it through the command, not by hand:
  `python3 -m governor doctor-baseline` lists what is unbaselined;
  `... --accept <code> --milestone M### --evidence "..."` records one review.
  There is no accept-all: `--code-wide` requires `--reason` and is the documented
  exception, and a code the latest run does not report is refused.
- the check is deliberately absent from `checks/assurance_severity.yaml`, so these
  signals produce `drift_risk`, never `blocked`: they describe the harness, and the
  project must see them without being unable to proceed on a race it cannot repair.
- `.gsd/` is a symlink out of the repository and is not tracked, so any artifact a
  human must review belongs under `scripts/governor/`, not under `.gsd/`.
- failed git actions are read from `.gsd/git-action-failures.log` and reported as
  `gsd_control_plane_commit_hook_failures` when they are newer than the last commit,
  with the recorded task verifications attached as the explanation. Only hooks whose
  `- hook id:` line is followed by `- exit code:` count as refusing: measured over all
  188 blocks, `docs-drift-check` appears in every one without refusing any. This is the
  detection half of the cargo-fmt pair; the prevention half is the two
  `pre_dispatch_hooks` entries described under control-plane discipline.

#### Control-plane discipline (rules with evidence)

These four come from measured failure, not preference. Each names the cost:

- **The same drift is repairable or terminal depending on who sees it first.** A
  formatting drift caught by the host verification gate is routed through the canonical
  recovery machine (`routeHostTechnicalFailure` -> retry/repair/remediate/replan, budget
  `verification_max_retries`, default 2, with `verification_auto_fix` recording the
  attempt count). The same drift caught by the `cargo-fmt` hook during the engine's
  auto-commit has **no retry at all** and kills the attempt with `LEASE_FENCING_LOST`.
  That asymmetry is why the fmt check belongs in a task's `verify`: it moves the
  detection from the terminal path to the repairable one. `verification_auto_fix` is
  not a formatter — it only annotates the evidence with the auto-fix attempt number,
  so nothing here formats on your behalf.
- **Run `cargo fmt` as the LAST action of a Rust task.** `cargo-fmt` caused 141 of
  188 recorded commit failures. This is not formatting cleanliness: a failed
  auto-commit cannot be retried by the engine, so the attempt dies with a claim
  mismatch and auto-mode wedges (`LEASE_FENCING_LOST`). Running fmt early in the task
  is the documented trap — any later test addition or artifact regeneration
  re-introduces drift, so the check must be the final step.
- **Recovery ladder when the auto-commit already failed** (a failed commit after the
  task was credited in the database cannot be retried; manual commit is mandatory,
  not optional):
  `cargo fmt --all` -> `cargo fmt --all --check` -> `git add -A && git commit`
  (let fmt/clippy/governor hooks pass) -> `gsd headless closeout resolve` (needs a
  clean tree) -> clear `paused_session` via `deleteRuntimeKv` -> fresh start.
- **This is now prevented mechanically, not by reminder.** `.gsd/PREFERENCES.md`
  carries two `pre_dispatch_hooks` entries: `rust-fmt-is-the-last-action` appends the
  obligation to every `execute-task` prompt, and `rust-task-verify-carries-fmt`
  appends the verify-field requirement to `plan-slice`. `action: modify` composes the
  text into the unit prompt at dispatch — no extra agent run, no extra model call — and
  the engine's preference cache is keyed on file `size:mtimeMs`, so an edit takes
  effect on the next dispatch without a restart. Verify with `resolvePreDispatchHooks`
  + `evaluatePreDispatch` from `rule-registry.js`.

  `.gsd/` is a symlink out of the repository and is not tracked, so this live config is
  not versioned. The snippet below is the canonical copy: if `.gsd/PREFERENCES.md` is
  lost or a fresh clone needs it, restore it from here.

  ```yaml
  pre_dispatch_hooks:
    - name: rust-fmt-is-the-last-action
      before: [execute-task]
      action: modify
      enabled: true
      append: >-
        If this task changed any Rust source (crates/**/*.rs), then `cargo fmt` must be
        the LAST thing you run before finishing -- after every edit, every added test,
        and every regenerated artifact. Run `cargo fmt -p <affected crate>` (or
        `cargo fmt --all`), confirm with `cargo fmt --all --check`, then `git add -A`.
        Running fmt earlier is not enough: any later edit re-introduces drift. A
        formatting drift that reaches the engine's auto-commit is not a style problem --
        the engine cannot retry a failed commit, the attempt dies with
        LEASE_FENCING_LOST, and the only way out is manual recovery.
    - name: rust-task-verify-carries-fmt
      before: [plan-slice]
      action: modify
      enabled: true
      append: >-
        For every task whose files include Rust sources, the task's `verify` must carry
        a rustfmt check. Put it on its own line in the verify field, for example:
        `cargo test -p <crate> --test <name>` on one line and
        `cargo fmt -p <crate> -- --check` on the next. Do not join commands with `&&` or
        `|`: the host verification gate rejects a verify line that is not shell-safe and
        then reports the task as unverified rather than running anything.
        This is not a style preference, and the reason is the whole point: a formatting
        drift that the HOST gate sees is routed through the canonical recovery machine
        and can be repaired and retried within the verification budget, while the same
        drift reaching the engine's auto-commit has no retry at all and kills the attempt
        with LEASE_FENCING_LOST. Verified on M511-j7wz2f/S02/T02 and again on M510. A
        task whose plan has no verify at all falls back to the project
        `verification_commands`, which includes fmt but says nothing about that task's
        actual work -- so a task-specific verify with fmt in it is the only real gate.
  ```

  The same shape applies to the project verification fallback:
  `.gsd/PREFERENCES.md#verification_commands` leads with `cargo fmt --all --check`
  because `discoverCommands` never appends that list to a task plan's own verify, so it
  is the only protection for a task whose plan carries no verify. Probe used to confirm
  it is a real check: rc=0 clean, rc=1 with one injected formatting drift.
- **Rejected alternatives (checked, do not re-litigate).** Switching the git hook to
  auto-format: a hook that modifies files makes `pre-commit run` exit 1 with "files
  were modified by this hook", so that commit is still refused once. Adding fmt to
  `verification_commands`: project-level commands are only a fallback —
  `discoverCommands` returns the task plan's own verify commands and never appends the
  preference list. A governor rule requiring fmt in every Rust task's verify: 44 of 54
  cargo-verified tasks in one week recorded no fmt check, so it would fire on 81% of
  Rust work and be ignored.
- **Treat `timed_out` as failure, whatever `exit_code` says.** The same review found
  six `gsd_exec` captures that reported `exit_code == 0` while `timed_out` was true.
  Long gates (full `governor architecture`, `--features pdf-layout` suites) do not
  fit the 600 s cap — run them detached and poll, do not tighten the cap.
- **A blocked auto exit is not always restartable; read the printed cause first.**
  Across 78 headless auto logs, `task-recovery-abort` / `recovery already aborted`
  appears in 12 (26 hits) and is the most common fatal signature: the engine cannot
  re-dispatch an aborted attempt, so restarting auto re-enters the same wedge. The
  engine prints the remedy — `/gsd recover <recoveryActionId>` or
  `gsd_task_recovery_resume` — and the watchdog restarted auto 16 times in 16 hours
  without clearing it. Provider stream death (`Anthropic stream ended before
  message_stop`, 10 logs) and reconciliation failure (5 logs) *are* restartable;
  `task-recovery-abort` is not.
- **Prune the runtime stores.** `python3 -m governor hygiene` reports sizes and
  prunes scratch/evidence; `--apply` deletes and writes a manifest to
  `.gsd/retention/`. The policy is DATA in `checks/gsd_control_plane.yaml#retention`
  (exec 30 d, quarantine 30 d, activity 14 d, scratch older than 24 h), so the
  threshold that fires and the command that clears it read one source.
- **A bloat signal must be clearable.** The control-plane check counts entries *past*
  their retention window, never total store size: an absolute limit on a store that
  grows daily is permanently red, and a flag nobody can clear is a flag nobody reads.

**Do NOT write new bash scripts to enforce invariants** the governor
already checks. Extend governor rules or `scripts/verify_rust_architecture.sh`
instead. `scripts/governor/` is the authoritative trajectory checker.

### Pre-commit hooks (`.pre-commit-config.yaml`)

First line of defense before CI. Covers kg-* + da-* in cargo-fmt and
cargo-clippy-lib (no `--all-targets`, avoids rocksdb rebuild), plus
cargo-clippy-tests (per-crate `--lib --tests`: clippy `--lib` does
not compile test modules, so a test-only break passes the lib hook and
fails under `cargo test`), governor-trajectory
(ADR invariants + artifact gates) and docs-drift-check.
Run automatically on `git commit`.

### GitNexus (code intelligence)

Reindex after structural changes (new crates, renamed symbols):
`node .gitnexus/run.cjs analyze`. Until then, new symbols stay unresolved.
Use `impact({target, direction:"upstream"})` before editing any symbol —
warn the user on HIGH/CRITICAL risk. Use `detect_changes()` before commit.

### Testing strategy (ADR-051)

**Two axes:** hexagonal layers (7 contours) × test types (8 categories).

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

See `doc/adr/ADR-051-testing-strategy-overlapping-verification-contours.md` for full details.

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
| Project state (GSD) | `.gsd/continue.md`, `.gsd/STATE.md`, `.gsd/PROJECT.md` |
| Binding ADRs | `doc/adr/ADR-INDEX.md` |
| Documented solutions (module / problem area) | `.compound-engineering/artifacts/solutions/` — YAML frontmatter (`module`, `tags`, `problem_type`); relevant when implementing or debugging in an area that already has an entry |
| Graph schema | `doc/GRAPH-SCHEMA.md` |
| Ontology alignment | `doc/ONTOLOGY-ALIGNMENT.md` |
| Healing scenarios | `doc/GRAPH-HEALING-SCENARIOS.md` |
| Persistence model | `doc/PERSISTENCE-ANALYSIS.md` |
| Extraction eval + corpus metrics | `data/gold_standard/`, `crates/da-cli/examples/eval_batch.rs` (output) |
| GROBID semantic correctness | `doc/adr/ADR-076-grobid-cascade-three-automata-graph-freeze.md`, `doc/review/grobid-absorption-followup.json` |

After a solved, verified problem, offer once to invoke the `ce-compound` skill at the completion checkpoint only when the work produced durable project reasoning that is not readily recoverable from the final code, tests, types, comments, or existing documentation, and losing it would plausibly cause recurrence, material risk, or substantial rediscovery. Apply this counterfactual: if the learning document disappeared, would a future engineer reading the final implementation still be likely to repeat the mistake or redo substantial investigation? If not, do not offer. Completion, effort, and diff size alone are not enough. Offer at the checkpoint so a qualifying learning can ship in the PR that produced it, and only where the repository treats captured learnings as tracked, committed knowledge.

Write every report, summary, or handoff to the user through the `ce-noslop` skill. This applies when you are the top-level agent writing to the user, not when you are a subagent reporting to its caller. Do not apply it to code, config, verbatim quotes, or text the user asked to post as written.

## Package map (Rust workspace)

| Crate | Path | Role |
|-------|------|------|
| Domain | `crates/da-domain/` | Paper/Entity/Evidence/Relation/Healing/Scheduler/Eval types |
| Ports | `crates/da-ports/` | GraphStore/DirectGraphStore (HOT path), ParserPort, Embedder, Extractor, OpenAlexClient |
| Application | `crates/da-application/` | Ingest/BatchIngest/Extraction/Healing/Enrich/Scheduler use-cases |
| Parse | `crates/da-parse/` | Scholarly PDF fulltext: Java GROBID HTTP (production route), native cascade shadow (ADR-076), TEI IR, ParserPort projector |
| Adapters | `crates/da-adapters/` | HTML parse, pdf-classifier, XY-Cut, fd_api embedder (bge-m3), SamyamaGraphStore, RuleBasedExtractor, OpenAlexHttpAdapter, GrobidParseStage |
| CLI | `crates/da-cli/` | `da` binary: health/version/ingest/batch-ingest/extract/schema-init/heal/enrich/scheduler-run |
| MCP | `crates/da-mcp/` | MCP server for knowledge-base external access (ADR-054) |
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

# Governor CLI — trajectory report (~60s: reverse-ADR audit and scholarly-plane
# check dominate; the control-plane check is ~0.5s)
PYTHONPATH=scripts python3 -m governor --phase preflight

# Runtime-store retention — reports by default, prunes only with --apply
PYTHONPATH=scripts python3 -m governor hygiene
PYTHONPATH=scripts python3 -m governor hygiene --apply

# Accepted engine-doctor divergence — list, then record one reviewed acceptance
PYTHONPATH=scripts python3 -m governor doctor-baseline
PYTHONPATH=scripts python3 -m governor doctor-baseline --accept <code> --milestone M### --evidence "..."

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

# ADR-051 P1 property-based tests
cargo test -p kg-ontology --lib proptest   # TemporalEdge intervals (8)
cargo test -p da-domain --lib proptest     # EntityType/RelationType/confidence/VID (19)
cargo test -p da-cli --lib proptest        # graph_tasks parser (6)

# ADR-051 §8.1 semantic graph task suite
cargo test -p da-cli --lib graph_tasks                        # parser unit
cargo run -p da-cli --example graph_task_suite -- \
  --snapshot artifacts/gepa_hub_production/hub.sgsnap \
  --tasks data/graph_tasks/gepa_hub.tasks.cypher \
  --report artifacts/graph_tasks/gepa_hub_report.json

# PDF OCR triage via pdf-inspector (optional feature)
cargo test -p da-adapters --features pdf-classifier --lib pdf_classifier
cargo test -p da-adapters --features pdf-classifier --test hub_ocr_probe -- --nocapture
# CLI build with pre-GROBID triage (off by default):
#   cargo build -p da-cli --features pdf-classifier

# Feature-gated shadow/contract tests (pdf-layout, off by default).
# Nine da-cli test files are `#![cfg(feature = "pdf-layout")]` because the
# examples they drive carry `required-features = ["pdf-layout"]`. A plain
# `cargo test -p da-cli --tests` compiles them to EMPTY targets, so their
# coverage is nominal unless the feature is enabled. `cargo test --test NAME`
# does NOT build examples, so provision the runners first:
cargo build -p da-cli --features pdf-layout --examples  # example runners
cargo test  -p da-cli --features pdf-layout --tests     # 447 tests (vs 418 default)
# `governor architecture` runs both as its last steps
# (da-cli feature-gated example runners / contracts).

# Offline stage-tape recorder for the Java GROBID cascade.
# Captures the model debug dump (GROBID `debugMode`) and derives per-model
# coverage: sections are invocations, not distinct models. Offline evidence
# only — it never writes a graph and is not part of `ParserPort`.
# Raw TEI is captured by the compatibility tooling, not here (one source of truth).
cargo run -p da-cli --example grobid_stage_tape -- \
  --pdf data/sources/arxiv/<cat>/<id>/source/<id>.pdf \
  --out /tmp/tape --endpoint fulltext --models segmentation
# `--models` filters the RESPONSE only; it does not reduce Java computation,
# and a filtered tape is marked `filtered` rather than `captured`.
# Verify a stored tape: re-hashes the source PDF and the stored dump, probes the
# live release, and exits non-zero on any finding (substitution, absence, drift,
# recorded failure, no sections). A tape nothing verifies is not evidence.
cargo run -p da-cli --example grobid_stage_tape -- --verify /tmp/tape/tape.json

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

# Multi-process RESP health (in-memory, no RocksDB)
# Port 16379 — not Redis 6379, not vendor SAMYAMA_RESP_PORT 6380
da health                              # reports DA_GRAPH_MODE; remote is a RESP read plane
da serve-graph --port 16379            # terminal A (optional: --snapshot file.sgsnap)
da graph-ping --port 16379             # terminal B → PONG
# DA_GRAPH_MODE=remote opens a read plane (RemoteGraphStore, crates/kg-storage/src/graph_mode.rs);
# Samyama HOT writes return GraphModeError::RemoteWritesUnsupported
```

Historical `legacy/scripts/verify_m0xx_*.py` — frozen Python milestone pins, not runtime ops. `legacy/` is gitignored (untracked frozen Python stack).

## Extraction eval baseline

Committed numbers (**2026-08-09**): **P=0.766, R=1.000, micro-F1=0.8677** on 200/200 fixtures, 1,439 gold entities (`archive/artifacts-legacy/gold_corpus/eval_aggregate_all_sources.json`). Remeasure with `cargo run -p da-cli --example eval_batch`; the committed artifact is the source of truth, not this line.

## Code navigation

1. Prefer **GitNexus** `query` / `context` / `impact` (rules below).
2. Before edit: `impact({target, direction:"upstream"})` — warn on HIGH/CRITICAL.
3. Before commit: `detect_changes({scope:"compare", base_ref:"main"})`.
4. Onion: `PYTHONPATH=scripts python3 -m governor architecture`.
5. Do not invent free labels; do not open import without user yes.

<!-- project-current:end -->

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **daily-archive** (84110 symbols, 136655 relationships, 652 execution flows).

> Index stale? Run `node .gitnexus/run.cjs analyze --index-only` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? Bootstrap with `npx`, `bunx`, or `pnpm dlx` — e.g. `bunx gitnexus@latest analyze` (npm 11 npx crash; #1939).

## Always Do

- **MUST run impact before editing.** Use `impact({target: "symbolName", direction: "upstream"})` or `node .gitnexus/run.cjs impact "symbolName" --direction upstream --repo .`; report callers, processes, and risk. Never substitute grep for graph analysis.
- **MUST analyze graph changes before committing.** Use `detect_changes({scope: "all"})` (MCP) or `node .gitnexus/run.cjs detect-changes --scope all --repo .` (CLI fallback). `partial: true` or `truncated: true` is not a clean check — a zero means unseen, not unaffected; re-run it. For regression review: `detect_changes({scope: "compare", base_ref: "main"})` or `node .gitnexus/run.cjs detect-changes --scope compare --base-ref "main" --repo .`.
- MUST warn on HIGH/CRITICAL `risk` pre-edit; never use `riskSharedAxes` to waive a HIGH/CRITICAL `risk` warning. Compare File/symbol: MCP File omits axes; Graph-RAG expands File.
- **MUST treat `risk: UNKNOWN` as unresolved, not as low.** An empty caller set is not evidence the symbol is unused — it can also mean the callers are not resolvable by the index (plain-object property access, dynamic dispatch, cross-language calls). `impact` pairs `UNKNOWN` with a `riskNote` saying so. Confirm with a text search before treating the symbol as safe to change or delete; do not proceed on the strength of a zero.
- **MUST use `query({search_query: "concept"})` for concepts/flows, `context({name: "symbolName"})` for a named symbol, or `impact` for blast radius, on read-only callers, dependencies, imports, or execution flow.** Graph first; text search only for empty/`UNKNOWN`/literals.
- For security review, `explain({target: "fileOrSymbol"})` lists taint findings (source→sink flows; needs `analyze --pdg`).

## Never Do

- NEVER edit a function, class, or method before MCP/CLI impact analysis.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis, and never read `UNKNOWN` as an all-clear — it means the walk could not answer, which is the one verdict that requires confirming by other means.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit before MCP/CLI graph change analysis.

## Resources

| Resource | Use for |
| --- | --- |
| `gitnexus://repo/daily-archive/context` | Codebase overview, check index freshness |
| `gitnexus://repo/daily-archive/clusters` | All functional areas |
| `gitnexus://repo/daily-archive/processes` | All execution flows |
| `gitnexus://repo/daily-archive/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
| --- | --- |
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus-cli/SKILL.md` |

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
LOG_SUPPRESS=lopdf::object=error   # extra target=level directives (default shown)
```

`LOG_SUPPRESS` is appended after the base filter and exists because a
third-party PDF parser (`lopdf`) emits one `WARN` per undecodable glyph or
font-encoding difference in real-world PDFs — observed ~2.5k lines/day across 19
distinct messages, which drowned our own signal and defeated the
query-the-log-by-level workflow below. It names an explicit third-party target
so no `da-*`/`kg-*` target is ever silenced, and uses `error` rather than `off`
so genuine PDF parse failures stay visible. Set it to an empty value to disable
suppression entirely.

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
