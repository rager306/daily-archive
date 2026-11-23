<!-- gsd-path:begin -->
# AGENTS.md — Operating Rules for the GSD Path Pipeline

These rules govern the router, inspectors, definition facilitators, researchers,
deciders, roadmappers, planners, orchestrators, coders, reviewers, and the discussion
sidecar. Role briefs are bundled with the installed skills. These rules win over role instructions except where the user says
otherwise.

## Authority order

1. The user, in chat.
2. `.project/CHARTER.md` (program flow): program scope, vetoes, and
   corrections bind every milestone. Vetoes may not be researched, planned,
   or built.
3. `.project/intent/INTENT.md`: constraints, vetoes, corrections, and success
   criteria are hard limits. Vetoes may not be researched, planned, or built.
   Only `$gsd-path-define` changes a success criterion, by appending
   `## Corrections`; a task Log or review cannot waive one.
4. `.project/SYNTHESIS.md` (program flow, top level) or
   `.project/research/SYNTHESIS.md` (single milestone): gated decisions are
   settled. Report conflicts; do not override them.
5. The current phase brief or task file.

If two sources disagree, stop and surface the conflict. Never average.

The orchestrator's isolated rerun of a task Verify is that task's evidence.
Wave and ship reviewers read that recorded output plus the isolated diff;
they do not re-run the task command. PLAN.md's project Verify runs once, at
ship, in one sidecar through `workflow_run.py prepare-final`. That runtime owns
execution, output recording, collection, and retry reuse. A complete quick-lane
single full wave with explicit final scope and walkthrough evidence may also
supply final review when the runtime proves unchanged product and contracts.
In that case FINAL.md is a generated view, not a new model assignment.
The project gap is always a view of its recorded command result.
Do not repeat a proven claim or write another narrative of the same evidence.
Verification effort follows uncovered contract claims and actual integration
risk; code line counts and token ratios are observations, never scope targets.
A task Verify must not copy the project command unless an
owned success criterion names it. Any other task Verify must name a path
from that task's `files`. INTENT constraints about not running the
full-repo suite on a tiny edit outrank the phase brief.

## Files are the only memory

- Start from disk, not conversation. If an input is absent from `.project/`,
  report it instead of inventing it.
- A discussion answer with `Status: final` or `NEEDS-USER`, `Follow-up:
  required`, and no later `Disposition X###` receipt is pending. Before phase
  work and again before a phase gate, the router and current phase run the
  active skill's bundled `scripts/discussion_records.py pending --repo
  <absolute-root>` helper; when `.project/discuss/` is absent it returns an
  empty list, so continue. Do not parse IDs, pair records, recover writes, or
  route pending receipts through model reasoning. The named owner either
  updates the target artifact through a legal current-phase gate and uses the
  helper's `dispose` command to append an `applied` disposition, or appends
  `acknowledged-no-change` with evidence. If applying it would rewrite an
  approved earlier-phase contract or the owner cannot legally enter, block the
  current phase with links to ANSWERS.md and the target artifact and ask the
  user; never auto-advance or archive it. Only the user may authorize
  `rejected-by-user`.
- After a phase completes its gate and state update, run the bundled
  `pipeline_state.py status --repo <absolute-root>` with the routed
  `--project-dir`. Its `handoff` is the shared phase handoff for chat and the
  dashboard. Present **Outcome** from `handoff.outcome` plus the completed
  work; **Review** links the phase artifact, or `handoff.review` when absent.
  An active router uses `handoff.router_next` for **Next** and follows the
  returned route. A directly invoked phase uses `handoff.phase_next` and
  stops. A status or plain-prompt reply uses `handoff.next` and stays read-only.
  Convert the `$` invocation prefix to the host's slash form when needed.
  Phase owners still stop for required input and approval before completion;
  a route is not approval. Return to an active caller instead of invoking an
  explicit-only sibling skill. Derive the next phase from this result, not
  from another lane table in prose.
- Phase skills bundle the remaining rules in `references/operating-rules.md`;
  read it before phase work.

## Plain-prompt re-entry

<!-- gsd-path/plain-prompt-reentry/v1 -->

- On a turn that did not explicitly invoke a GSD Path skill, check for
  `.project/STATE.md`. When it exists, first select the compatible interpreter:
  probe `python3 -B -c "import sys; raise SystemExit(sys.version_info < (3, 9))"`,
  then probe `python -B -c "import sys; raise SystemExit(sys.version_info < (3, 9))"`
  only if the first command fails. Run the first successful interpreter
  with `-B .gsd-path/status_runtime.py --repo <absolute-root>`
  before any requested repository mutation. Treat its JSON as the only route
  authority.
  When `completion.status` is `verified` and `git.branch` is an ordinary
  branch (not `gsd-path/M###`), requested product work may proceed normally.
  Archives and pipeline control files remain protected. Otherwise,
  a plain change request does not authorize work outside the pipeline: make no
  changes and report **Outcome**, link the returned `path` under **Review**, and
  under **Next** use `handoff.next`. An explicit request to leave
  or bypass the pipeline is a user ruling;
  route it through the router's undo or abandon flow instead of editing directly.
- After any other plain-prompt turn with owned state, rerun the same read-only
  status command immediately before the final response and append the same
  **Outcome** / **Review** / **Next** handoff with the same shared handoff rule.
  Keep an informational turn read-only from start to finish: do not use repository
  mutation tools or run commands that can create or alter files. A GSD Path skill
  already supplies this handoff, so emit it once. With no STATE.md, respond
  normally and do not initialize the pipeline. A missing runtime or invalid
  status blocks mutation and routes to `$gsd-path-forensics`.

## Evidence and honesty

- Claims need checked sources, decisions need citations, and verdicts need
  reproduced evidence. Never report a command as passing unless it ran.
- Report partial or failed results plainly. Silent partial success is the
  worst outcome.
- Preserve uncertainty as `NEEDS-USER`, `RESEARCH`, or a confidence level.
  Questions never silently disappear between phases.
- Record user corrections and vetoes verbatim.

## Gates

- Same-wave dependencies execute in dependency order; a task dispatches as
  soon as its dependencies land, never idling behind unrelated
  in-flight tasks. Only ready tasks run.
- Parallel dispatch rounds use distinct linked worktrees, each created at the
  clean primary HEAD recorded as its task base at dispatch. A serial dispatch
  round (one ready task) works and lands on the bound branch in the primary
  worktree. Task and reviewer Verify run against that recorded base plus only
  the task patch; evidence from a shared worktree or combined branch tip does
  not count.
- A wave advances only after every task and the wave review pass.
- Final review blocks on any `not-met`, `unverifiable`, or blocked gap verdict.
- STATE.md becomes `shipped` only when every final verdict and the project
  verify pass; the router reports shipped only after the integration
  validator (`validate-integrated`) passes.
- Shipping archives the milestone: artifacts move to
  `.project/archive/<NNN>-<slug>/` with a manifest, and the ship phase
  records the ship commit (STATE.md, final reviews, archive) as its single
  commit on the bound branch. Archives are read-only —
  no agent may modify or delete them — and a new milestone may not begin
  while an un-archived shipped milestone's artifacts sit in the active paths.
- STATE.archive is the crash-recovery transaction id. It is persisted before
  moves and never recomputed. Keep review active until the prepared archive,
  canonical contents, carry-forward, and manifest pass the bundled precommit
  validator; report shipped only after the exact `.project/`-only ship commit
  passes the postcommit validator and `validate-integrated` proves the
  integration landing, its tag, and its ancestry on origin/main —
  while integration is pending the router routes back to ship instead of
  reporting shipped or starting the next milestone.

## Asking the user

- Ask through an interactive user-input tool when the runtime provides one;
  otherwise ask concise numbered questions in chat and stop for the reply.
- Before any approval, ruling, `NEEDS-USER` question, blocked escalation, or
  phase-completion handoff, present three things in order: **Outcome** — what
  was produced or learned; **Review** — a Markdown link to the primary
  canonical artifact using its resolved absolute path; **Next** — the one
  question or action now required. Any text the user is expected to send back
  verbatim — a ruling, an approval command, a reply — goes in its own fenced
  code block, never a blockquote or inline prose, so it pastes cleanly. If the
  host cannot render local links, print
  the resolved absolute path immediately after the link. On an output failure,
  link the malformed artifact when it exists; otherwise link STATE.md or the
  canonical log that proves the failure and say the expected artifact is
  missing. Never ask for approval before its reviewable artifact exists on
  disk, and never ask a bare "approve?" without its outcome and link.
- Every choice offered to the user names a recommended option — listed first
  and marked `(recommended)` — with a one-line reason grounded in evidence,
  intent, or the codebase, followed by the real alternatives. A pure values
  call with no evidence either way carries no recommendation; say so
  explicitly instead of inventing one.
- After a user decision, confirm what changed, link the updated artifact again,
  and state whether the active router continues automatically or which exact
  explicit skill the user should invoke next.

## Escalation

Stop and ask the user only when:

- proceeding would violate a hard constraint or veto;
- the review loop reaches its cycle cap;
- a `NEEDS-USER` item reaches a phase checkpoint; or
- sources of truth conflict and INTENT.md does not make the resolution clear.

Handle everything else autonomously and record it in the task log or
STATE.md.

## Style

- Write fixed-format data for the next agent, without pleasantries.
- Write short, plain-English outcomes for the user.
- Keep agent final messages to paths, statuses, verdicts, and evidence.

## Distribution layout

This section describes the GSD Path source checkout. These source paths and
the sync command do not apply to an application that only installs GSD Path.
In a consuming project, resolve roles, templates, and helpers from the active
installed skill's absolute paths; the application need not contain this layout.

| Path | Purpose |
|------|---------|
| `plugin.json` | Agent Plugins manifest (`agent-plugins.org` 1.0.0 schema) |
| `skills/` | Skills and aliases declared in `scripts/skill-resources.json` |
| `skills/gsd-path/templates/` | Required artifact formats |
| `skills/gsd-path/references/` | Agent role and dispatch contracts |
| `WORKFLOW.md` | Phase-by-phase SOP |

Edit canonical resources only: `skills/gsd-path/` templates and references,
each per-skill `SKILL.md`, `scripts/`, and
`platforms/shared-agents/dispatch.md`. Every per-skill `references/`,
`templates/`, and `scripts/` copy is generated — run `python3
scripts/sync_skill_resources.py` after editing a canonical source. Sync
overwrites divergent generated copies and warns when a divergent copy is
newer than its canonical source (the wrong-direction-edit signature); treat
that warning as a lost edit and re-apply it to the canonical path.
<!-- gsd-path:end -->

<!-- project-current:start -->
# daily-archive — agent map (what / where)

Runtime: **Rust workspace** `crates/da-{domain,ports,application,parse,adapters,cli,mcp}` (hexagonal / onion).  
Frozen evidence: Python stack cold-archived under `archive/python-era-cold-*/` (see `archive/PYTHON-ERA-COLD.md`); `legacy/` is a restore stub — not on the runtime path.

## Workflow engine (cutover)

GSD planning state is frozen: `.gsd/` (gsd-pi) no longer governs this project; do not call `gsd_*` engine tools. Active engine: **gsd-path** — `.project/` state, `gsd-path*` skills via omp (`/skill:gsd-path`, explicit-only). Migration bundle + IMPORT.md: `/root/da-path-migration-20261122`. Historical gsd-pi detail: `AGENTS.pre-path.md`. Rulings 2026-11-22: M521 carries into Path as first intent; `.project/` is committed by pipeline checkpoint commits; the pre-migration user snapshot noted at migration time is historical, not current state.

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
 cargo check  -p <changed-crate>   # per-crate dev loop
 cargo clippy -p <changed-crate> --all-targets --no-deps -- -D warnings
 cargo test   -p <changed-crate> --lib   # NEVER cargo test --workspace: rocksdb rebuild 5-10 min
```

`PYTHONPATH=scripts python3 -m governor architecture` is the **full gate** — run it
only as the final check at slice end, not on every edit. It runs
fmt + clippy + hexagonal invariant + per-crate tests for all our crates.

**Concurrent cargo processes block on `target/` file lock.** Cancel
background `async_bash` cargo jobs (via `cancel_job`) before re-running.
Stale `async_bash` results surface as delayed "background job done"
notices — their content is often already superseded.

### Governor CLI (trajectory enforcement)

`scripts/governor/` is the governor CLI: trajectory report from ADRs/dimensions/flags; rules are DATA in `checks/*.yaml`.

```bash
PYTHONPATH=scripts python3 -m governor --phase preflight   # default
python3 scripts/check_project_trajectory.py --phase active
PYTHONPATH=scripts python3 -m governor --phase closeout
PYTHONPATH=scripts python3 -m governor hygiene             # dry run; --apply to prune
```

Reverse-ADR audit enforces 65 rules (see `scripts/governor/checks/adr_rules.yaml`):
artifacts-gate (D127/ADR-005/ADR-040), Rust-specific invariants (kg-* must not
depend on da-*, da-domain must not depend
on infra, da-application must not depend on da-adapters in prod deps) and the
full declarative rule set.
Negative test: inject `da-domain` into a kg-* Cargo.toml → governor flips
to `verdict=blocked`.

`no_new_product_python_scripts` (D237) forbids new Python under `scripts/`
except the governance surface. Two allowlisted roots exist: `scripts/governor/`
and `scripts/tests/`. A new tool elsewhere under `scripts/` is a hard finding —
put it in `scripts/governor/` (or `checks/*.yaml` as DATA) instead.

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
# Export all 43 node schemas to YAML
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

## Where to look first

| Question | Open this |
|----------|-----------|
| Human overview + architecture | `README.md` |
| Binding ADRs | `doc/adr/ADR-INDEX.md` |
| Graph schema | `doc/GRAPH-SCHEMA.md` |
| Ontology alignment | `doc/ONTOLOGY-ALIGNMENT.md` |
| Extraction eval + corpus metrics | `data/gold_standard/`, `crates/da-cli/examples/eval_batch.rs` |
| GROBID semantic correctness | ADR-076 + `doc/review/grobid-absorption-followup.json` |
| Documented solutions | `.compound-engineering/artifacts/solutions/` (`module`, `tags`, `problem_type`) |
| Project state | `AGENTS.pre-path.md` (frozen gsd-pi era), `.gsd/` frozen planning archive |


After a solved problem, offer `ce-compound` once only if the durable reasoning would be re-derived (would a future engineer repeat the mistake from the final code/tests/docs?). User-facing reports go through `ce-noslop`.

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
| **kg-\*** | `crates/kg-{ontology,storage,embeddings,algorithms,pipeline}/` | Universal ADR-050 layer: types, storage traits, embeddings, algorithms, pipeline DSL. Must NOT depend on da-* (governor-enforced). |
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

# Feature-gated tests (pdf-layout): plain `cargo test -p da-cli --tests` compiles them EMPTY; provision runners first:
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


## Extraction eval baseline


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
# GROBID CRF (recommended; no API key)
docker compose -f .docker/docker-compose.yml --env-file .env up -d grobid
curl -sS http://127.0.0.1:8070/api/isalive   # true
docker compose -f .docker/docker-compose.yml --env-file .env down
```
Env keys: `.env.example` (`GROBID_*`, `HYBRID_AUTO_START_CONTAINERS`). REST profiles (D204): `rich` = consolidate 1 (compat; minutes under Crossref limits); `structural` = consolidate 0 (managed default). Never mix profiles in one cache key; profile is part of the parse fingerprint.
```

Auto-ensure from Rust: `GrobidParser::from_env()` + `is_alive().await`. Probe/start is infrastructure — `import_eligible` stays false; sidecar TEI/MD feeds the Rust `RuleBasedExtractor` (canonical entities), never graph truth. Details: `.docker/README.md`.
<!-- parser-sidecars:end -->

<!-- logging:start -->
# Structured Logging (ADR-052)

Dual-layer tracing: stderr (compact) + rolling JSON `logs/da.log.YYYY-MM-DD` under
`RUST_LOG`/`LOG_LEVEL` (`.env`: `LOG_DIR`, `LOG_LEVEL`, `LOG_SUPPRESS`; the default suppresses
`lopdf::object=error` — third-party PDF glyph noise, ~2.5k lines/day; no `da-*`/`kg-*` target is ever silenced).
Query: `jq 'select(.paper_id=="…")' logs/da.log.<date>` (also `.level`, `.span.name`, `.fields.duration_ms`).
`init_logging()` returns `Option<WorkerGuard>` — hold it in `main()` scope; dropping it early silently stops file logging.
Owner: da-cli subscriber; `#[instrument]` on da-application use cases and da-parse `parse_pdf`; pure crates (da-domain, da-ports, kg-*) carry zero tracing.
<!-- logging:end -->

